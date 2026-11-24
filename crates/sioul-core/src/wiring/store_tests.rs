// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The rules' list: a round trip; a rule or a part of a rule this Sioul does
//! not know kept byte for byte and never run; a newer format read-only; a
//! rule that cannot run kept off; models, general and a project's. Invented
//! names only.

use super::store::*;
use std::collections::BTreeMap;

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("sioul-wiring-store-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn tax_rule() -> Rule {
    Rule {
        on: true,
        when: When {
            kind: "message".into(),
            event: "arrives".into(),
            join: Join::All,
            conditions: vec![Condition { field: "sender-domain".into(), test: "is".into(), value: vec!["impots.example".into()], until: String::new() }, Condition { field: "kind".into(), test: "is".into(), value: vec!["bill".into()], until: String::new() }],
        },
        then: Then { kind: "message".into(), action: "keep-attachments".into(), with: BTreeMap::from([("type".to_string(), Param::Text("pdf".into())), ("title".to_string(), Param::Text("{sender} bill, {month}".into()))]) },
        ..Rule::default()
    }
}

/// Written and read again, the same rule: its "when" and "then" as dotted
/// keys, its conditions one per line, an id and the moment it acts from
/// given; every view of rules told.
#[test]
fn a_rule_is_written_and_read_again() {
    let dir = scratch("round");
    let path = dir.join(FILE);
    let (id, said) = crate::changes::heard(|| Rules::change(&path, |rules| rules.put(tax_rule(), 1_795_000_000)).unwrap().unwrap());
    assert_eq!(said, [crate::changes::Kind::Rules]);
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.starts_with("# Your rules"), "{text}");
    assert!(text.contains("version = 1") && text.contains("when.type = \"message\"") && text.contains("\n    { field = \"sender-domain\", test = \"is\", value = [\"impots.example\"] },"), "{text}");
    let read = Rules::load(&path).unwrap();
    let rule = read.get(&id).unwrap();
    assert_eq!((rule.since, rule.on, rule.unknown.is_empty()), (1_795_000_000, true, true));
    assert_eq!(rule.when, tax_rule().when);
    assert_eq!(rule.then, tax_rule().then);
    assert!(rule.runs(), "{:?}", rule.problem());
    // Nothing changed: nothing written.
    let before = std::fs::read_to_string(&path).unwrap();
    let ((), said) = crate::changes::heard(|| Rules::change(&path, |_| ()).unwrap());
    assert!(said.is_empty());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), before);
    // As JSON, the window's shape.
    let json = serde_json::to_value(rule).unwrap();
    assert_eq!(json["when"]["if"][0]["field"], "sender-domain");
    assert_eq!(json["then"]["with"]["title"], "{sender} bill, {month}");
    let back: Rule = serde_json::from_value(json).unwrap();
    assert_eq!(back.when, rule.when);
    let _ = std::fs::remove_dir_all(&dir);
}

/// What a newer Sioul wrote (an event this one does not know, a key of its
/// own, an action it does not know in a rule written as tables) is kept
/// byte for byte when another rule changes, never run, never edited here;
/// turned off, its `on` alone is written into it. Comments and other keys
/// of the file stay.
#[test]
fn what_this_sioul_does_not_know_is_kept_byte_for_byte() {
    let dir = scratch("unknown");
    let path = dir.join(FILE);
    let newer = "[[rule]]\nid = \"newer\"\non = true\nsince = 1795000000\nwhen.type = \"message\"\nwhen.event = \"is-forwarded\"   # a later event\nwhen.if = [ { field = \"subject\", test = \"has-words\", value = [\"x\"] } ]\nthen.type = \"none\"\nthen.action = \"tell\"\ncolour = \"blue\"\n";
    let tables = "[[rule]]\nid = \"tables\"\non = true\nsince = 1795000000\n\n[rule.when]\ntype = \"message\"\nevent = \"arrives\"\nif = [{ field = \"subject\", test = \"has-words\", value = [\"y\"] }]\n\n[rule.then]\ntype = \"message\"\naction = \"paint\"\n";
    let text = format!("# Mine.\nversion = 1\nlater = {{ kept = true }}\n\n{newer}\n{tables}");
    std::fs::write(&path, &text).unwrap();
    let rules = Rules::load(&path).unwrap();
    assert_eq!(rules.rules.len(), 2);
    assert!(rules.rules[0].unknown.iter().any(|u| u.contains("is-forwarded")) && rules.rules[0].unknown.iter().any(|u| u == "colour"), "{:?}", rules.rules[0].unknown);
    assert!(rules.rules[1].unknown.iter().any(|u| u.contains("paint")), "{:?}", rules.rules[1].unknown);
    assert!(rules.running().next().is_none(), "never run here");
    // Another rule added, the newer one turned off, an edit of it refused.
    let mut edited = rules.rules[0].clone();
    edited.then.action = "give-topic".into();
    let refused = Rules::change(&path, |rules| {
        rules.put(tax_rule(), 1_795_100_000).unwrap();
        assert!(rules.set_on("newer", false, 1_795_100_000));
        rules.put(edited, 1_795_100_000)
    })
    .unwrap();
    assert_eq!(refused, Err("rules-problem-unknown".to_string()), "an unknown rule is never edited here");
    let after = std::fs::read_to_string(&path).unwrap();
    assert!(after.starts_with("# Mine.\nversion = 1\nlater = { kept = true }\n"), "{after}");
    assert!(after.contains(&newer.replace("on = true", "on = false")), "{after}");
    assert!(after.contains("[rule.when]\ntype = \"message\"\nevent = \"arrives\"\nif = [{ field = \"subject\", test = \"has-words\", value = [\"y\"] }]\n"), "{after}");
    assert!(after.contains("[rule.then]\ntype = \"message\"\naction = \"paint\"\n"), "{after}");
    let again = Rules::load(&path).unwrap();
    assert_eq!(again.rules.len(), 3);
    assert!(!again.get("newer").unwrap().on);
    // One removed, the others stay as they were.
    Rules::change(&path, |rules| rules.remove("tables")).unwrap();
    let last = std::fs::read_to_string(&path).unwrap();
    assert!(!last.contains("paint") && last.contains("colour = \"blue\""), "{last}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A list a newer format wrote is read and shown, and nothing in it runs
/// nor is written here; a file that does not read is never written over.
#[test]
fn a_newer_format_is_read_only() {
    let dir = scratch("newer");
    let path = dir.join(FILE);
    let text = "version = 2\n\n[[rule]]\nid = \"a\"\non = true\nwhen.type = \"message\"\nwhen.event = \"arrives\"\nwhen.if = [{ field = \"subject\", test = \"has-words\", value = [\"x\"] }]\nthen.type = \"none\"\nthen.action = \"tell\"\n";
    std::fs::write(&path, text).unwrap();
    let rules = Rules::load(&path).unwrap();
    assert!(rules.read_only() && rules.running().next().is_none());
    assert_eq!(Rules::change(&path, |rules| rules.remove("a")).unwrap_err(), "rules-newer-format");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
    std::fs::write(&path, "[[rule]\nid =").unwrap();
    assert!(Rules::change(&path, |rules| rules.put(tax_rule(), 1)).is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "[[rule]\nid =");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A rule that cannot run is kept off, and says why; one of a later phase
/// waits; turned on again, it acts from then on; a model of its own and a
/// project's are both read as models.
#[test]
fn a_rule_that_cannot_run_is_kept_off() {
    let dir = scratch("off");
    let path = dir.join(FILE);
    let no_condition = Rule { when: When { conditions: Vec::new(), ..tax_rule().when }, ..tax_rule() };
    assert_eq!(no_condition.problem(), Some("rules-problem-no-condition"));
    let later = Rule { when: When { event: "is-sent".into(), ..tax_rule().when }, ..tax_rule() };
    assert_eq!(later.problem(), Some("rules-problem-coming"));
    let trash = Rule { then: Then { kind: "message".into(), action: "trash".into(), with: BTreeMap::new() }, ..tax_rule() };
    assert_eq!(trash.problem(), Some("rules-problem-forbidden"));
    let no_topic = Rule { then: Then { kind: "message".into(), action: "give-topic".into(), with: BTreeMap::new() }, ..tax_rule() };
    assert_eq!(no_topic.problem(), Some("rules-problem-param"));
    let id = Rules::change(&path, |rules| rules.put(no_topic.clone(), 10)).unwrap().unwrap();
    assert!(!Rules::load(&path).unwrap().get(&id).unwrap().on, "kept off");
    let mut fixed = no_topic;
    fixed.id = id.clone();
    fixed.on = false;
    fixed.then.with.insert("topic".into(), Param::Text("taxes".into()));
    Rules::change(&path, |rules| rules.put(fixed, 20).unwrap()).unwrap();
    assert!(Rules::change(&path, |rules| rules.set_on(&id, true, 30)).unwrap());
    let rule = Rules::load(&path).unwrap().get(&id).cloned().unwrap();
    assert_eq!((rule.on, rule.since), (true, 30), "turned on, it acts from then on");
    let model = Model { name: "Pay a bill".into(), prefix: "Pay:".into(), business_days: Some(3), ..Model::default() };
    let model_id = Rules::change(&path, |rules| rules.put_model(model)).unwrap().unwrap();
    assert_eq!(model_id, "pay-a-bill");
    // A reader's or a preset's rule is never stored as yours: theirs run in their own modules.
    for id in ["read:project/p/route/r1/task", "preset:codes-told"] {
        assert_eq!(Rules::change(&path, |rules| rules.put(Rule { id: id.into(), ..tax_rule() }, 40)).unwrap(), Err("rules-problem-reserved".to_string()));
    }
    let rules = Rules::load(&path).unwrap();
    assert_eq!(rules.models[0].prefix, "Pay:");
    let project = crate::projects::Project { id: "flat".into(), title: "Flat".into(), models: vec![crate::projects::Model { id: "visit".into(), name: "Visit".into(), ..crate::projects::Model::default() }], ..crate::projects::Project::default() };
    let store = crate::projects::ProjectStore { root: dir.clone(), notes: dir.clone(), projects: vec![project], ties: Default::default() };
    let all = models(&rules, Some(&store));
    assert_eq!(all.iter().map(|m| (m.id.as_str(), m.editable)).collect::<Vec<_>>(), [("pay-a-bill", true), ("flat/visit", false)]);
    let _ = std::fs::remove_dir_all(&dir);
}

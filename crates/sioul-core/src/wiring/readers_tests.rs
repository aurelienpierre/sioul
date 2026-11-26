// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! What exists, read as rules: a project with routes and a model, a
//! budget's mail rule, a mail filter, a topic's sender rule; each in the
//! shape of a stored rule, with where it is edited today; none of them runs
//! in the engine. And the presets: their rules in the same shape, the ones
//! offered now runnable. Invented names only.

use super::declare;
use super::readers::*;
use super::store::Param;
use crate::projects::{Model, Project, ProjectStore, Route};

fn text(param: Option<&Param>) -> String {
    param.and_then(Param::text).unwrap_or_default()
}

/// A project's routes read as "tie it to this project", a route that makes
/// tasks as "make a task from its model" too, each saying it is edited in
/// the project.
#[test]
fn a_project_s_routes_and_model_read_as_rules() {
    let route = Route { id: "r1".into(), from_domains: vec!["shop.example.org".into()], subject_contains: vec!["New sale".into()], makes: "send".into(), ..Route::default() };
    let other = Route { text_contains: vec!["devis".into()], attachment_contains: vec!["facture".into()], ..Route::default() };
    let project = Project { id: "gift-cards".into(), title: "Gift cards".into(), routes: vec![route, other], models: vec![Model { id: "send".into(), name: "Send a gift card".into(), ..Model::default() }], ..Project::default() };
    let store = ProjectStore { root: "/nowhere".into(), notes: "/nowhere".into(), projects: vec![project], ties: Default::default() };
    let read = of_projects(&store);
    assert_eq!(read.iter().map(|r| (r.rule.id.as_str(), r.source.kind, r.source.edited)).collect::<Vec<_>>(), [
        ("read:project/gift-cards/route/r1", "project-route", "project"),
        ("read:project/gift-cards/route/r1/task", "route-task", "project"),
        ("read:project/gift-cards/route/1", "project-route", "project"),
    ]);
    let tie = &read[0].rule;
    assert_eq!(tie.when.conditions.iter().map(|c| (c.field.as_str(), c.test.as_str(), c.value.join(","))).collect::<Vec<_>>(), [("sender-domain", "is", "shop.example.org".to_string()), ("subject", "has-words", "New sale".to_string())]);
    assert_eq!((tie.then.kind.as_str(), tie.then.action.as_str(), text(tie.then.with.get("project"))), ("message", "tie-to-project", "gift-cards".to_string()));
    let make = &read[1].rule;
    assert_eq!((make.then.kind.as_str(), make.then.action.as_str(), text(make.then.with.get("model"))), ("task", "make", "gift-cards/send".to_string()));
    assert_eq!(make.then.with.get("title"), Some(&Param::From { from: "subject".into() }));
    assert!(read.iter().all(|r| r.rule.unknown.is_empty() && r.rule.problem().is_none()), "the same shape as a stored rule, which could run");
    assert_eq!(read[2].rule.when.conditions.iter().map(|c| c.field.as_str()).collect::<Vec<_>>(), ["text", "attachment"]);
}

/// A budget's mail rule: "plan a movement in this budget", its direction,
/// its amount (the message's when none is fixed); a mail filter, one rule
/// per action, its accounts a condition, its days as days; a topic's
/// sender rule, Always and Never.
#[test]
fn a_mail_rule_a_filter_and_a_sender_rule_read_as_rules() {
    let ledger: crate::budget::Ledger = toml::from_str("[[mail_rule]]\nbudget = 'shop'\ndirection = 'credit'\nfrom_domains = ['pay.example']\nsubject_contains = ['paiement reçu']\nautomatic = true\n\n[[mail_rule]]\nbudget = 'home'\ndirection = 'debit'\nfrom_addresses = ['bills@power.example']\namount = 42.5\n").unwrap();
    let rules = of_mail_rules(&ledger);
    assert_eq!(rules.len(), 2);
    let (credit, debit) = (&rules[0], &rules[1]);
    assert_eq!((credit.rule.then.kind.as_str(), credit.rule.then.action.as_str()), ("movement", "plan"));
    assert_eq!((text(credit.rule.then.with.get("budget")), text(credit.rule.then.with.get("direction"))), ("shop".to_string(), "credit".to_string()));
    assert_eq!(credit.rule.then.with.get("amount"), Some(&Param::From { from: "amount".into() }));
    assert!(credit.source.automatic && credit.source.edited == "budgets");
    assert_eq!(text(debit.rule.then.with.get("amount")), "42.50");
    assert_eq!(credit.rule.problem(), Some("rules-problem-coming"), "planning a movement comes in phase 3");
    let filters: crate::config::Config = toml::from_str("[[mail.filter]]\nname = \"Bank\"\naccounts = [\"home\"]\nmatch = \"any\"\nstop = true\nif = [\n  { field = \"from\", test = \"contains\", value = \"@bank.example\" },\n  { field = \"weekday\", test = \"is\", value = \"sat,sun\" },\n]\nthen = [\n  { do = \"move\", folder = \"Banque\" },\n  { do = \"read\" },\n]\n").unwrap();
    let read = of_filters(&filters.mail.filters);
    assert_eq!(read.iter().map(|r| (r.rule.then.action.as_str(), r.source.filter, r.source.stop)).collect::<Vec<_>>(), [("move-to-folder", Some(0), true), ("mark-read", Some(0), true)]);
    let when = &read[0].rule.when;
    assert_eq!(when.join, super::store::Join::Any);
    assert_eq!(when.conditions.iter().map(|c| (c.field.as_str(), c.value.clone())).collect::<Vec<_>>(), [("from", vec!["@bank.example".to_string()]), ("weekday", vec!["sat".to_string(), "sun".to_string()]), ("account", vec!["home".to_string()])]);
    assert_eq!(text(read[0].rule.then.with.get("folder")), "Banque");
    assert_eq!(read[0].source.edited, "mail-filters");
    let said = crate::topics::labels::Said::of(&[
        crate::topics::labels::Act::Sender { at: 1, address: "news@choir.example".into(), topic: "choir".into(), rule: crate::topics::labels::Rule::Always },
        crate::topics::labels::Act::Sender { at: 2, address: "news@choir.example".into(), topic: "tax".into(), rule: crate::topics::labels::Rule::Never },
    ]);
    let senders = of_senders(&said);
    assert_eq!(senders.iter().map(|r| (r.rule.then.action.as_str(), text(r.rule.then.with.get("topic")))).collect::<Vec<_>>(), [("give-topic", "choir".to_string()), ("never-topic", "tax".to_string())]);
    assert_eq!(senders[0].rule.when.conditions[0].field, "sender");
    let all = read_all(None, Some(&ledger), &filters.mail.filters, &said);
    assert_eq!(all.len(), 6);
    assert!(all.iter().all(|r| r.rule.id.starts_with("read:")));
}

/// The presets: what Sioul does by itself, in the shape of a rule, each
/// with its reference and when it moves into the engine; the
/// ones offered now give a draft that runs, off, remembering its preset.
#[test]
fn the_presets_say_what_sioul_does_by_itself() {
    let presets = declare::presets();
    assert!(presets.len() >= 40);
    let mut ids: Vec<&str> = presets.iter().map(|p| p.id).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), presets.len(), "stable ids, each once");
    for preset in &presets {
        assert!(preset.rule.unknown.is_empty() && declare::of(&preset.rule.when.kind).and_then(|t| t.event(&preset.rule.when.event)).is_some(), "{}", preset.id);
        assert!(declare::of(&preset.rule.then.kind).and_then(|t| t.action(&preset.rule.then.action)).is_some(), "{}", preset.id);
        assert_eq!(preset.runs, "as-today");
    }
    let offered: Vec<&str> = presets.iter().filter(|p| p.offered).map(|p| p.id).collect();
    assert!(offered.is_empty(), "the one the engine could run, codes told, is never offered beside its own notice: {offered:?}");
    let codes = presets.iter().find(|p| p.id == "codes-told").unwrap();
    assert!(codes.rule.problem().is_none() && codes.phase == 1, "it could run");
    assert!(declare::from_preset("codes-told").is_none() && declare::from_preset("supplies-used").is_none(), "shown, not offered");
    let money = presets.iter().find(|p| p.id == "money-mail-planned").unwrap();
    assert_eq!((money.rule.then.kind.as_str(), money.rule.then.action.as_str(), money.phase), ("movement", "plan", 3));
    assert_eq!(presets.iter().find(|p| p.id == "new-mail-told").unwrap().setting, Some("reminders.mail"));
}

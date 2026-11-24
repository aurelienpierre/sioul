// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The engine: once on every device (two stand-in devices, one task; an act
//! undone on one never redone on another), the guards (a first fetch, what
//! came before the rule, mail set aside, a sender nothing proves, your own
//! mail), the chain, the preview that writes nothing, each of the five
//! actions, and attachments kept as papers (once across two devices, never
//! past the antivirus, the name's inserts, Undo). Invented names only: a
//! shop at shop.example.org.

use super::acts::{Act, Acts};
use super::engine::*;
use super::store::{Condition, Join, Model, Param, Rule, Rules, Then, When};
use crate::config::{Config, Priority, Source};
use crate::i18n::Translator;
use crate::links::LocalLinks;
use crate::porch::Senders;
use crate::projects::{Project, ProjectStore};
use crate::vdir::{Collection, Kind};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const GENUINE: &str = "spf=pass smtp.mailfrom=shop.example.org; dkim=pass header.d=shop.example.org; dmarc=pass header.from=shop.example.org";
const FORGED: &str = "spf=fail smtp.mailfrom=attacker.example; dkim=none; dmarc=fail header.from=shop.example.org";
const FAILED: &str = "spf=fail smtp.mailfrom=shop.example.org; dkim=fail header.d=shop.example.org";

/// A shop's mail folder, a task list, a project, the papers' folder: what a
/// stand-in device holds. Two devices share the mail they fetch (each its
/// own copy) and, once it travelled, the record of acts.
struct Device {
    dir: PathBuf,
    name: &'static str,
    mail: PathBuf,
    list: Collection,
    /// The record of acts every device reads: this device's, or one shared.
    acts: PathBuf,
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("sioul-wiring-engine-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

impl Device {
    fn new(base: &Path, name: &'static str) -> Device {
        let dir = base.join(name);
        let mail = dir.join("mail");
        std::fs::create_dir_all(mail.join("cur")).unwrap();
        std::fs::create_dir_all(dir.join("work")).unwrap();
        let list = Collection { kind: Kind::Calendars, account: "local".into(), id: "work".into(), dir: dir.join("work"), name: "Work".into(), color: None, read_only: false, components: vec!["VTODO".into()] };
        Device { acts: dir.join("acts"), dir, name, mail, list }
    }

    /// A message stored in its mail folder, come on `on` at 10:00 UTC (its
    /// file's time and its Date), `attachment` a PDF's name when it carries one.
    fn store(&self, n: u32, from: &str, results: &str, subject: &str, on: &str, attachment: Option<&str>) -> PathBuf {
        let path = self.mail.join("cur").join(format!("{n}.U1-{n}.sioul{}2,S", crate::maildir::INFO));
        let results = if results.is_empty() { String::new() } else { format!("Authentication-Results: mx.provider.example; {results}\r\n") };
        let at = format!("{on}T10:00").parse::<jiff::civil::DateTime>().unwrap().to_zoned(jiff::tz::TimeZone::UTC).unwrap();
        let head = format!("{results}Message-ID: <sale-{n}@shop.example.org>\r\nFrom: {from}\r\nTo: you@you.example.org\r\nDate: {}\r\nSubject: {subject}\r\nMIME-Version: 1.0\r\n", at.strftime("%a, %d %b %Y %H:%M:%S %z"));
        let body = match attachment {
            None => "Content-Type: text/plain\r\n\r\nPrint: To Ann, 50 EUR.\r\n".to_string(),
            Some(name) => format!(
                "Content-Type: multipart/mixed; boundary=\"b\"\r\n\r\n--b\r\nContent-Type: text/plain\r\n\r\nYour bill is attached.\r\n--b\r\nContent-Type: application/pdf; name=\"{name}\"\r\nContent-Disposition: attachment; filename=\"{name}\"\r\nContent-Transfer-Encoding: base64\r\n\r\nJVBERi0xLjQKJSBhIGJpbGwgb2YgdGhlIHNob3AK\r\n--b--\r\n"
            ),
        };
        std::fs::write(&path, format!("{head}{body}")).unwrap();
        let time = std::time::UNIX_EPOCH + std::time::Duration::from_secs(u64::try_from(at.timestamp().as_second()).unwrap());
        std::fs::File::options().write(true).open(&path).unwrap().set_modified(time).unwrap();
        path
    }

    fn place(&self) -> Place {
        Place { device: self.name.into(), acts: self.acts.clone(), ties: self.dir.join("links.toml"), topics: self.dir.join("topics") }
    }

    fn places(&self) -> crate::storage::Places {
        crate::storage::Places::under(&self.dir)
    }

    /// This device's engine with `rules`, its antivirus saying `scan`.
    fn engine(&self, rules: &[Rule], scan: Scanned) -> Engine<'static> {
        let now = "2026-11-23T12:00".parse::<jiff::civil::DateTime>().unwrap().to_zoned(jiff::tz::TimeZone::UTC).unwrap();
        let mut list = Rules::default();
        list.rules = rules.to_vec();
        list.models = vec![Model { id: "send".into(), name: "Send a gift card".into(), prefix: "Send:".into(), business_days: Some(3), estimate: Some(15), ..Model::default() }];
        let project = Project { id: "gift-cards".into(), title: "Gift cards".into(), ..Project::default() };
        let store = ProjectStore { root: self.dir.clone(), notes: self.dir.clone(), projects: vec![project], ties: Default::default() };
        let config = Config { topics: vec![crate::topics::Topic { id: "shop".into(), name: "Shop".into(), ..crate::topics::Topic::default() }], ..Config::default() };
        let mut engine = Engine::of(&config, self.name, &now, list, Some(store), Senders::default(), Vec::new());
        engine.sources = vec![Source { account: Some("home".into()), address: Some("you@you.example.org".into()), folder: self.mail.clone(), trusted_ids: vec!["mx.provider.example".into()], priority: Priority::Average, shielded: false, words: crate::words::Words::builtin().into(), spam: None }];
        engine.own = vec!["you@you.example.org".into()];
        engine.place = self.place();
        engine.acts = Acts::read(&self.acts);
        engine.places = Some(self.places());
        engine.scratch = self.dir.join("scratch");
        engine.workdays = crate::routetasks::Workdays::new(&[], crate::vacations::DaysOff::default());
        let list = self.list.clone();
        engine.lists = Box::new(move |_: &Model| Some(list.clone()));
        let list = self.list.clone();
        engine.find_list = Box::new(move |_: &str| Some(list.clone()));
        engine.topic_of = Box::new(|_| None);
        engine.scan = Box::new(move |_: &Path| scan.clone());
        engine
    }

    fn arrive(&self, rules: &[Rule], files: &[PathBuf], first: bool) -> Outcome {
        let engine = self.engine(rules, Scanned::Clean);
        let judged = engine.judge(files);
        engine.on_arrival(&judged, first, &Translator::new("en"))
    }

    fn ties(&self) -> LocalLinks {
        LocalLinks::load(&self.dir.join("links.toml"))
    }

    fn tasks(&self) -> Vec<PathBuf> {
        std::fs::read_dir(&self.list.dir).unwrap().map(|e| e.unwrap().path()).collect()
    }
}

fn condition(field: &str, test: &str, value: &str) -> Condition {
    Condition { field: field.into(), test: test.into(), value: vec![value.into()], until: String::new() }
}

fn rule(id: &str, conditions: Vec<Condition>, kind: &str, action: &str, with: &[(&str, Param)]) -> Rule {
    Rule {
        id: id.into(),
        on: true,
        since: 0,
        when: When { kind: "message".into(), event: "arrives".into(), join: Join::All, conditions },
        then: Then { kind: kind.into(), action: action.into(), with: with.iter().map(|(k, v)| ((*k).to_string(), v.clone())).collect::<BTreeMap<_, _>>() },
        ..Rule::default()
    }
}

fn sale_task() -> Rule {
    rule("r-task", vec![condition("sender-domain", "is", "example.org"), condition("subject", "has-words", "New sale")], "task", "make", &[("model", Param::Text("send".into()))])
}

/// Two stand-in devices run the same rule on the same message: before
/// either read the other's acts, both name the task alike (one task once
/// their lists meet on the server; where the file is there already, none
/// written); once the record travelled, the second does nothing. The task:
/// its title from the subject, the rule's words off, the model's prefix in
/// front; asked three business days after the mail; tied to its message;
/// the rule's id on it; nothing of the mail's text.
#[test]
fn two_devices_running_one_rule_make_one_task() {
    crate::config::stand_ins_only();
    let base = scratch("once");
    let (desk, phone) = (Device::new(&base, "desk"), Device::new(&base, "phone"));
    let at_desk = desk.store(1, "Shop <shop@shop.example.org>", GENUINE, "[New sale] Gift card 50 €", "2026-11-20", None);
    let at_phone = phone.store(1, "Shop <shop@shop.example.org>", GENUINE, "[New sale] Gift card 50 €", "2026-11-20", None);
    let first = desk.arrive(&[sale_task()], std::slice::from_ref(&at_desk), false);
    assert!(first.problems.is_empty() && first.refused.is_empty(), "{first:?}");
    assert_eq!(first.done.len(), 1);
    let uid = task_uid("r-task", "mid:sale-1@shop.example.org");
    assert_eq!(first.done[0].act.result, crate::links::task_uri(&uid));
    let text = std::fs::read_to_string(desk.list.dir.join(format!("{uid}.ics"))).unwrap();
    let task = crate::tasks::task_of_text(&text, &jiff::tz::TimeZone::UTC).unwrap();
    assert_eq!((task.title.as_str(), task.due.as_str(), task.estimate), ("Send: Gift card 50 €", "2026-11-25", 15));
    assert!(text.contains("X-SIOUL-RULE:r-task") && text.contains("X-SIOUL-ORIGIN:mid:sale-1@shop.example.org") && !text.contains("To Ann"), "{text}");
    assert_eq!(first.done[0].said.files.len(), 1, "its Undo knows the file");
    assert!(desk.ties().links.iter().any(|e| e.how == "made" && e.from == crate::links::task_uri(&uid)));
    // The phone, the desk's acts not here yet: the same UID.
    let second = phone.arrive(&[sale_task()], std::slice::from_ref(&at_phone), false);
    assert_eq!(second.done.iter().map(|d| d.act.result.clone()).collect::<Vec<_>>(), [crate::links::task_uri(&uid)]);
    // The same list (the server's copy came): found there, nothing written.
    std::fs::remove_dir_all(&phone.list.dir).unwrap();
    std::fs::create_dir_all(&phone.list.dir).unwrap();
    std::fs::copy(desk.list.dir.join(format!("{uid}.ics")), phone.list.dir.join(format!("{uid}.ics"))).unwrap();
    std::fs::remove_dir_all(&phone.acts).unwrap();
    let found = phone.arrive(&[sale_task()], std::slice::from_ref(&at_phone), false);
    assert_eq!(found.done.iter().map(|d| d.act.wrote).collect::<Vec<_>>(), [false]);
    assert_eq!(phone.tasks().len(), 1);
    // The desk's record travelled to the phone: nothing at all.
    let travelled = Device { acts: desk.acts.clone(), ..Device::new(&base, "tablet") };
    let tablet_mail = travelled.store(1, "Shop <shop@shop.example.org>", GENUINE, "[New sale] Gift card 50 €", "2026-11-20", None);
    let none = travelled.arrive(&[sale_task()], &[tablet_mail], false);
    assert!(none.done.is_empty() && travelled.tasks().is_empty(), "{none:?}");
    let _ = std::fs::remove_dir_all(&base);
}

/// An act undone on one device is put back there, said "not for this one",
/// and never redone on another device that reads its record, nor on the
/// same; a task changed since stays, and is still never made again.
#[test]
fn an_act_undone_is_not_redone_by_another_device() {
    crate::config::stand_ins_only();
    let base = scratch("undone");
    let desk = Device::new(&base, "desk");
    let mail = desk.store(1, "Shop <shop@shop.example.org>", GENUINE, "[New sale] Gift card", "2026-11-20", None);
    let done = desk.arrive(&[sale_task()], std::slice::from_ref(&mail), false);
    let act = done.done[0].act.clone();
    let list = desk.list.clone();
    let undone = undo(&act, &desk.place(), Some(&desk.places()), &move |_| Some(list.clone()), 1_795_500_000_000).unwrap();
    assert!(undone.put_back);
    assert!(desk.tasks().is_empty(), "the task went");
    assert!(!desk.ties().links.iter().any(|e| e.how == "made"), "its tie too");
    assert!(Acts::read(&desk.acts).undone("r-task", "mid:sale-1@shop.example.org"));
    let phone = Device { acts: desk.acts.clone(), ..Device::new(&base, "phone") };
    let at_phone = phone.store(1, "Shop <shop@shop.example.org>", GENUINE, "[New sale] Gift card", "2026-11-20", None);
    assert!(phone.arrive(&[sale_task()], &[at_phone], false).done.is_empty());
    assert!(phone.tasks().is_empty());
    assert!(desk.arrive(&[sale_task()], &[mail], false).done.is_empty());
    // Changed since: it stays.
    let other = Device::new(&base, "other");
    let mail = other.store(2, "Shop <shop@shop.example.org>", GENUINE, "[New sale] Card", "2026-11-20", None);
    let act = other.arrive(&[sale_task()], &[mail], false).done[0].act.clone();
    let path = other.tasks()[0].clone();
    std::fs::write(&path, std::fs::read_to_string(&path).unwrap().replace("SUMMARY:", "SUMMARY:Mine ")).unwrap();
    let list = other.list.clone();
    let undone = undo(&act, &other.place(), Some(&other.places()), &move |_| Some(list.clone()), 1).unwrap();
    assert!(!undone.put_back && path.exists());
    let _ = std::fs::remove_dir_all(&base);
}

/// The guards: nothing at an address's first fetch; nothing on what came
/// before the rule; nothing on mail the Porch sets aside (forged); a sender
/// condition never holds for a sender nothing proves, a subject condition
/// does; nothing on your own mail.
#[test]
fn a_rule_never_acts_where_the_guards_say_no() {
    crate::config::stand_ins_only();
    let base = scratch("guards");
    let desk = Device::new(&base, "desk");
    let genuine = desk.store(1, "Shop <shop@shop.example.org>", GENUINE, "[New sale] One", "2026-11-20", None);
    assert!(desk.arrive(&[sale_task()], std::slice::from_ref(&genuine), true).done.is_empty(), "a first fetch");
    let later = Rule { since: 1_900_000_000, ..sale_task() };
    assert!(desk.arrive(&[later], std::slice::from_ref(&genuine), false).done.is_empty(), "before the rule");
    let forged = desk.store(2, "Shop <shop@shop.example.org>", FORGED, "[New sale] Two", "2026-11-20", None);
    let unproven = desk.store(3, "Shop <shop@shop.example.org>", FAILED, "[New sale] Three", "2026-11-20", None);
    let own = desk.store(4, "Me <you@you.example.org>", "", "[New sale] Four", "2026-11-20", None);
    let by_sender = desk.arrive(&[sale_task()], &[forged.clone(), unproven.clone(), own.clone()], false);
    assert!(by_sender.done.is_empty(), "{by_sender:?}");
    let by_subject = rule("r-subject", vec![condition("subject", "has-words", "New sale")], "task", "make", &[("model", Param::Text("send".into()))]);
    let done = desk.arrive(&[by_subject], &[forged, unproven, own], false);
    assert_eq!(done.done.iter().map(|d| d.act.object.as_str()).collect::<Vec<_>>(), ["mid:sale-3@shop.example.org"], "unproven, by its subject alone");
    let _ = std::fs::remove_dir_all(&base);
}

/// A chain stops: a rule never acts on what it made itself, nor past three
/// steps from the first object.
#[test]
fn the_chain_stops() {
    crate::config::stand_ins_only();
    let base = scratch("chain");
    let desk = Device::new(&base, "desk");
    let mail = desk.store(1, "Shop <shop@shop.example.org>", GENUINE, "[New sale] One", "2026-11-20", None);
    let object = "mid:sale-1@shop.example.org";
    let made = |rule: &str, depth: u8| Act { at: 1, rule: rule.into(), object: "sioul:task/x".into(), act: "task".into(), result: object.into(), wrote: true, depth, ..Act::default() };
    super::acts::append(&super::acts::own_log(&desk.acts, "desk"), &[made("r-task", 1)]).unwrap();
    let own = desk.arrive(&[sale_task()], std::slice::from_ref(&mail), false);
    assert_eq!(own.refused.iter().map(|r| r.why).collect::<Vec<_>>(), ["rules-refused-own"]);
    std::fs::remove_dir_all(&desk.acts).unwrap();
    super::acts::append(&super::acts::own_log(&desk.acts, "desk"), &[made("r-other", 3)]).unwrap();
    let chained = desk.arrive(&[sale_task()], std::slice::from_ref(&mail), false);
    assert_eq!(chained.refused.iter().map(|r| r.why).collect::<Vec<_>>(), ["rules-refused-chain"]);
    assert!(chained.done.is_empty());
    std::fs::remove_dir_all(&desk.acts).unwrap();
    super::acts::append(&super::acts::own_log(&desk.acts, "desk"), &[made("r-other", 2)]).unwrap();
    let third = desk.arrive(&[sale_task()], &[mail], false);
    assert_eq!(third.done.iter().map(|d| d.act.depth).collect::<Vec<_>>(), [3], "the third step, the last");
    let _ = std::fs::remove_dir_all(&base);
}

/// The preview says what a rule would do over the mail handed, and writes
/// nothing: no task, no tie, no act, no topic's word, no paper.
#[test]
fn the_preview_does_nothing() {
    crate::config::stand_ins_only();
    let base = scratch("preview");
    let desk = Device::new(&base, "desk");
    let files = [desk.store(1, "Shop <shop@shop.example.org>", GENUINE, "[New sale] One", "2026-11-20", Some("bill.pdf")), desk.store(2, "Shop <shop@shop.example.org>", GENUINE, "Weekly news", "2026-11-21", None)];
    let tr = Translator::new("en");
    for then in [
        sale_task(),
        rule("r-tie", vec![condition("sender-domain", "is", "example.org")], "message", "tie-to-project", &[("project", Param::Text("gift-cards".into()))]),
        rule("r-topic", vec![condition("sender-domain", "is", "example.org")], "message", "give-topic", &[("topic", Param::Text("shop".into()))]),
        rule("r-paper", vec![condition("sender-domain", "is", "example.org")], "message", "keep-attachments", &[]),
    ] {
        let engine = desk.engine(&[], Scanned::Clean);
        let would = engine.preview(&then, &engine.judge(&files), &tr);
        assert!(!would.is_empty(), "{}", then.id);
        assert!(would.iter().all(|w| !w.done_already && w.refused.is_none()), "{would:?}");
    }
    assert!(desk.tasks().is_empty() && !desk.dir.join("links.toml").exists() && !desk.acts.exists() && !desk.dir.join("topics").exists() && !desk.dir.join("papers").exists() && !desk.dir.join(crate::papers::MANIFEST).exists());
    let _ = std::fs::remove_dir_all(&base);
}

/// Tie it to a project, give it a topic, tell: each once; the tie the
/// routes write; your own word on a topic stands; a notice only from the
/// device that keeps the notices, never a code in it.
#[test]
fn a_tie_a_topic_and_a_notice() {
    crate::config::stand_ins_only();
    let base = scratch("acts");
    let desk = Device::new(&base, "desk");
    let mail = desk.store(1, "Shop <shop@shop.example.org>", GENUINE, "Your code is 482913", "2026-11-20", None);
    let tie = rule("r-tie", vec![condition("sender-domain", "is", "example.org")], "message", "tie-to-project", &[("project", Param::Text("gift-cards".into()))]);
    let topic = rule("r-topic", vec![condition("sender-domain", "is", "example.org")], "message", "give-topic", &[("topic", Param::Text("shop".into()))]);
    let tell = rule("r-tell", vec![condition("kind", "is", "code")], "none", "tell", &[("level", Param::Text("now".into()))]);
    let mut engine = desk.engine(&[tie.clone(), topic.clone(), tell.clone()], Scanned::Clean);
    engine.tells_here = true;
    let out = engine.on_arrival(&engine.judge(std::slice::from_ref(&mail)), false, &Translator::new("en"));
    assert_eq!(out.done.iter().map(|d| d.act.act.as_str()).collect::<Vec<_>>(), ["tie", "topic", "tell"], "{out:?}");
    assert!(desk.ties().links.iter().any(|e| e.how == "project" && e.from == "mid:sale-1@shop.example.org" && e.to == "sioul:project/gift-cards"));
    let said = crate::topics::labels::Said::of(&crate::topics::labels::read_all_in(&desk.dir.join("topics")));
    assert_eq!(said.message("sale-1@shop.example.org"), Some(Some("shop")));
    assert_eq!(out.notices.len(), 1);
    assert!(!out.notices[0].body.contains("482913"), "{}", out.notices[0].body);
    // Undo of the topic: your word taken back.
    let act = out.done.iter().find(|d| d.act.act == "topic").unwrap().act.clone();
    undo(&act, &desk.place(), None, &|_| None, 1_796_000_000_000).unwrap();
    assert_eq!(crate::topics::labels::Said::of(&crate::topics::labels::read_all_in(&desk.dir.join("topics"))).message("sale-1@shop.example.org"), None);
    // Another device that does not keep the notices tells nothing, and records nothing for it.
    let phone = Device::new(&base, "phone");
    let at_phone = phone.store(1, "Shop <shop@shop.example.org>", GENUINE, "Your code is 482913", "2026-11-20", None);
    let quiet = phone.arrive(&[tell], std::slice::from_ref(&at_phone), false);
    assert!(quiet.notices.is_empty() && quiet.done.is_empty());
    // Your own word on the topic stands.
    crate::topics::labels::append(&crate::topics::labels::own_log(&phone.dir.join("topics"), "phone"), &crate::topics::labels::tagging(&["sale-1@shop.example.org".to_string()], None, 1)).unwrap();
    let yours = phone.arrive(&[topic], &[at_phone], false);
    assert_eq!(yours.refused.iter().map(|r| r.why).collect::<Vec<_>>(), ["rules-refused-your-word"]);
    let _ = std::fs::remove_dir_all(&base);
}

/// Attachments kept in Papers: the PDF, named by the rule's pattern, tied
/// to its message; once across two devices (the second finds the paper
/// the wallet brought, and ties it); never past an antivirus that refuses
/// (nor ever after, on any device), nor where none answered; Undo takes the
/// paper and its file out; the message is never changed.
#[test]
fn attachments_are_kept_as_papers_once() {
    crate::config::stand_ins_only();
    let base = scratch("papers");
    let desk = Device::new(&base, "desk");
    let mail = desk.store(1, "Shop <shop@shop.example.org>", GENUINE, "Your bill", "2026-11-20", Some("bill_november.pdf"));
    let message = std::fs::read(&mail).unwrap();
    let keep = rule("r-paper", vec![condition("sender-domain", "is", "example.org")], "message", "keep-attachments", &[("title", Param::Text("{sender} bill, {month}".into())), ("kind", Param::Text("invoice".into()))]);
    let out = desk.arrive(std::slice::from_ref(&keep), std::slice::from_ref(&mail), false);
    assert_eq!(out.done.len(), 1, "{out:?}");
    let act = out.done[0].act.clone();
    assert!(act.wrote);
    let wallet = crate::papers::Wallet::load_at(&desk.places()).unwrap();
    assert_eq!(wallet.papers.len(), 1);
    let paper = &wallet.papers[0];
    assert_eq!(paper.title, "Shop bill, November 2026");
    let file = wallet.file_path(paper).unwrap();
    assert!(std::fs::read(&file).unwrap().starts_with(b"%PDF-1.4"));
    assert!(desk.ties().links.iter().any(|e| e.how == "made" && e.from == crate::links::paper_uri(&paper.id) && e.to == "mid:sale-1@shop.example.org"));
    assert_eq!(std::fs::read(&mail).unwrap(), message, "the message never changes");
    // A second device, the wallet and its folder come with the sharing, its acts not yet.
    let phone = Device::new(&base, "phone");
    std::fs::create_dir_all(phone.dir.join("papers")).unwrap();
    std::fs::copy(desk.dir.join(crate::papers::MANIFEST), phone.dir.join(crate::papers::MANIFEST)).unwrap();
    std::fs::copy(&file, phone.dir.join("papers").join(file.file_name().unwrap())).unwrap();
    let at_phone = phone.store(1, "Shop <shop@shop.example.org>", GENUINE, "Your bill", "2026-11-20", Some("bill_november.pdf"));
    let again = phone.arrive(std::slice::from_ref(&keep), std::slice::from_ref(&at_phone), false);
    assert_eq!(again.done.iter().map(|d| d.act.wrote).collect::<Vec<_>>(), [false], "{again:?}");
    assert_eq!(crate::papers::Wallet::load_at(&phone.places()).unwrap().papers.len(), 1);
    assert_eq!(std::fs::read_dir(phone.dir.join("papers")).unwrap().count(), 1);
    // A file kept by hand already: the rule ties it, keeps nothing more.
    let tablet = Device::new(&base, "tablet");
    std::fs::create_dir_all(tablet.dir.join("papers")).unwrap();
    std::fs::copy(&file, tablet.dir.join("papers").join("mine.pdf")).unwrap();
    crate::papers::Wallet::change_at(&tablet.places(), |w| w.put(crate::papers::Paper { id: "mine".into(), title: "Mine".into(), file: "papers/mine.pdf".into(), ..crate::papers::Paper::default() })).unwrap();
    let at_tablet = tablet.store(1, "Shop <shop@shop.example.org>", GENUINE, "Your bill", "2026-11-20", Some("bill_november.pdf"));
    let by_hand = tablet.arrive(std::slice::from_ref(&keep), &[at_tablet], false);
    assert_eq!(by_hand.done.iter().map(|d| (d.act.result.as_str(), d.act.wrote)).collect::<Vec<_>>(), [("sioul:paper/mine", false)]);
    // Undo: the paper and its file go back out.
    let undone = undo(&act, &desk.place(), Some(&desk.places()), &|_| None, 3).unwrap();
    assert!(undone.put_back);
    assert!(crate::papers::Wallet::load_at(&desk.places()).unwrap().papers.is_empty() && !file.exists());
    // An antivirus that refuses: nothing kept, here nor anywhere after.
    let infected = Device::new(&base, "infected");
    let bad = infected.store(1, "Shop <shop@shop.example.org>", GENUINE, "Your bill", "2026-11-20", Some("bill.pdf"));
    let engine = infected.engine(std::slice::from_ref(&keep), Scanned::Infected("Eicar".into()));
    let refused = engine.on_arrival(&engine.judge(std::slice::from_ref(&bad)), false, &Translator::new("en"));
    assert_eq!(refused.refused.iter().map(|r| r.why).collect::<Vec<_>>(), ["rules-refused-infected"]);
    assert!(crate::papers::Wallet::load_at(&infected.places()).unwrap().papers.is_empty());
    assert!(Acts::read(&infected.acts).undone("r-paper", "mid:sale-1@shop.example.org"), "never kept after");
    let engine = infected.engine(std::slice::from_ref(&keep), Scanned::NoAntivirus);
    assert!(engine.on_arrival(&engine.judge(&[bad]), false, &Translator::new("en")).done.is_empty());
    // None answered (a computer without an antivirus): nothing kept, nothing recorded.
    let unchecked = Device::new(&base, "unchecked");
    let mail = unchecked.store(1, "Shop <shop@shop.example.org>", GENUINE, "Your bill", "2026-11-20", Some("bill.pdf"));
    let engine = unchecked.engine(std::slice::from_ref(&keep), Scanned::Unanswered("none".into()));
    let out = engine.on_arrival(&engine.judge(&[mail]), false, &Translator::new("en"));
    assert_eq!(out.refused.iter().map(|r| r.why).collect::<Vec<_>>(), ["rules-refused-unchecked"]);
    assert!(Acts::read(&unchecked.acts).all.is_empty());
    let _ = std::fs::remove_dir_all(&base);
}

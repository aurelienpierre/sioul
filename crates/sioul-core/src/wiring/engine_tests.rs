// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The engine: once on every device (two stand-in devices, one task, the
//! same bytes; an act undone on one taken back on the other, even before the
//! record travelled), the guards (a first fetch, what came before the rule,
//! mail more than a week old, mail set aside, held in the screener or
//! unproven, your own mail), the chain, the preview that writes nothing,
//! each of the five actions, what a rule makes kept with its message's
//! project and never given a code, the ties arriving mail takes to its
//! project, pending refusals tried again, and
//! attachments kept as papers (once across two devices, two files of one
//! name, hostile names, a phone keeping unchecked and a computer refusing,
//! types read from the bytes, Undo). Invented names only: a shop at
//! shop.example.org.

use super::acts::{Act, Acts};
use super::engine::*;
use super::store::{Condition, Join, Model, Param, Rule, Rules, Then, When};
use crate::config::{Config, Priority, Source};
use crate::i18n::Translator;
use crate::links::LocalLinks;
use crate::porch::{SenderList, Senders};
use crate::projects::{Project, ProjectStore, Route};
use crate::vdir::{Collection, Kind};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const GENUINE: &str = "spf=pass smtp.mailfrom=shop.example.org; dkim=pass header.d=shop.example.org; dmarc=pass header.from=shop.example.org";
const FORGED: &str = "spf=fail smtp.mailfrom=attacker.example; dkim=none; dmarc=fail header.from=shop.example.org";
const FAILED: &str = "spf=fail smtp.mailfrom=shop.example.org; dkim=fail header.d=shop.example.org";
const STRANGER: &str = "spf=pass smtp.mailfrom=other.example; dkim=pass header.d=other.example; dmarc=pass header.from=other.example";

/// "%PDF-1.4\n% a bill of the shop\n", and another PDF.
const PDF: &str = "JVBERi0xLjQKJSBhIGJpbGwgb2YgdGhlIHNob3AK";
const OTHER_PDF: &str = "JVBERi0xLjQKJSBhbm90aGVyIGJpbGwK";
/// "<html><body>a page</body></html>"
const HTML: &str = "PGh0bWw+PGJvZHk+YSBwYWdlPC9ib2R5PjwvaHRtbD4=";

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
    /// The closed project's routes: none, or the shop's mail into it.
    routes: Vec<Route>,
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("sioul-wiring-engine-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// 23 November 2026, noon UTC: the devices' now.
fn noon() -> jiff::Zoned {
    "2026-11-23T12:00".parse::<jiff::civil::DateTime>().unwrap().to_zoned(jiff::tz::TimeZone::UTC).unwrap()
}

impl Device {
    fn new(base: &Path, name: &'static str) -> Device {
        let dir = base.join(name);
        let mail = dir.join("mail");
        std::fs::create_dir_all(mail.join("cur")).unwrap();
        std::fs::create_dir_all(dir.join("work")).unwrap();
        let list = Collection { kind: Kind::Calendars, account: "local".into(), id: "work".into(), dir: dir.join("work"), name: "Work".into(), color: None, read_only: false, components: vec!["VTODO".into()] };
        Device { acts: dir.join("acts"), dir, name, mail, list, routes: Vec::new() }
    }

    /// A message stored in its mail folder, come on `on` at 10:00 UTC (its
    /// file's time and its Date), with its attachments (name, type, base64).
    fn store(&self, n: u32, from: &str, results: &str, subject: &str, on: &str, attachments: &[(&str, &str, &str)]) -> PathBuf {
        let path = self.mail.join("cur").join(format!("{n}.U1-{n}.sioul{}2,S", crate::maildir::INFO));
        let results = if results.is_empty() { String::new() } else { format!("Authentication-Results: mx.provider.example; {results}\r\n") };
        let at = format!("{on}T10:00").parse::<jiff::civil::DateTime>().unwrap().to_zoned(jiff::tz::TimeZone::UTC).unwrap();
        let head = format!("{results}Message-ID: <sale-{n}@shop.example.org>\r\nFrom: {from}\r\nTo: you@you.example.org\r\nDate: {}\r\nSubject: {subject}\r\nMIME-Version: 1.0\r\n", at.strftime("%a, %d %b %Y %H:%M:%S %z"));
        let body = if attachments.is_empty() {
            "Content-Type: text/plain\r\n\r\nPrint: To Ann, 50 EUR.\r\n".to_string()
        } else {
            let mut body = "Content-Type: multipart/mixed; boundary=\"b\"\r\n\r\n--b\r\nContent-Type: text/plain\r\n\r\nYour bill is attached.\r\n".to_string();
            for (name, mime, data) in attachments {
                body.push_str(&format!("--b\r\nContent-Type: {mime}; name=\"{name}\"\r\nContent-Disposition: attachment; filename=\"{name}\"\r\nContent-Transfer-Encoding: base64\r\n\r\n{data}\r\n"));
            }
            body + "--b--\r\n"
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

    /// This device's engine with `rules`, its antivirus saying `scan`; the
    /// shop let in (its mail in People); one project, "secret", closed.
    fn engine(&self, rules: &[Rule], scan: Scanned) -> Engine<'static> {
        let mut list = Rules::default();
        list.rules = rules.to_vec();
        list.models = vec![Model { id: "send".into(), name: "Send a gift card".into(), prefix: "Send:".into(), business_days: Some(3), estimate: Some(15), ..Model::default() }];
        let projects = vec![Project { id: "gift-cards".into(), title: "Gift cards".into(), ai: true, ..Project::default() }, Project { id: "secret".into(), title: "Secret".into(), ai: false, routes: self.routes.clone(), ..Project::default() }];
        let store = ProjectStore { root: self.dir.clone(), notes: self.dir.clone(), projects, ties: Default::default() };
        let config = Config { topics: vec![crate::topics::Topic { id: "shop".into(), name: "Shop".into(), ..crate::topics::Topic::default() }], ..Config::default() };
        let senders = Senders { known: SenderList::parse("*@shop.example.org"), ..Senders::default() };
        let mut engine = Engine::of(&config, self.name, &noon(), list, Some(store), senders, Vec::new());
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
        let list = self.list.clone();
        engine.all_lists = Box::new(move || vec![list.clone()]);
        engine.topic_of = Box::new(|_| None);
        let mail = self.mail.clone();
        engine.find_mail = Box::new(move |address: &str| {
            let n = address.strip_prefix("mid:sale-")?.split('@').next()?.to_string();
            std::fs::read_dir(mail.join("cur")).ok()?.filter_map(Result::ok).map(|e| e.path()).find(|p| p.file_name().is_some_and(|f| f.to_string_lossy().starts_with(&format!("{n}."))))
        });
        engine.scan = Box::new(move |_: &Path| scan.clone());
        engine
    }

    fn arrive(&self, rules: &[Rule], files: &[PathBuf], first: bool) -> Outcome {
        self.arrive_scanned(rules, files, first, Scanned::Clean)
    }

    fn arrive_scanned(&self, rules: &[Rule], files: &[PathBuf], first: bool, scan: Scanned) -> Outcome {
        let engine = self.engine(rules, scan);
        let judged = engine.judge(files);
        engine.on_arrival(&judged, first, &Translator::new("en"))
    }

    fn ties(&self) -> LocalLinks {
        LocalLinks::load(&self.dir.join("links.toml"))
    }

    fn tasks(&self) -> Vec<PathBuf> {
        std::fs::read_dir(&self.list.dir).unwrap().map(|e| e.unwrap().path()).collect()
    }

    fn wallet(&self) -> crate::papers::Wallet {
        crate::papers::Wallet::load_at(&self.places()).unwrap()
    }

    /// Another device's record of acts copied here, as the sharing brings it.
    fn receive_acts(&self, from: &Device) {
        std::fs::create_dir_all(&self.acts).unwrap();
        for entry in std::fs::read_dir(&from.acts).unwrap().filter_map(Result::ok) {
            if entry.file_name().to_string_lossy() != format!("{}.jsonl", self.name) {
                std::fs::copy(entry.path(), self.acts.join(entry.file_name())).unwrap();
            }
        }
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

fn keep_papers(with: &[(&str, Param)]) -> Rule {
    rule("r-paper", vec![condition("sender-domain", "is", "example.org")], "message", "keep-attachments", with)
}

/// Two stand-in devices run the same rule on the same message: before
/// either read the other's acts, both write the same bytes under the same
/// UID (stamped from the message's arrival, never their clocks), so that
/// their servers see one task and each one's Undo knows it; where the file
/// is there already, none written; once the record travelled, the second
/// does nothing. The task: its title from the subject, the rule's words
/// off, the model's prefix in front; asked three business days after the
/// mail; tied to its message; the rule's id on it; nothing of the mail's text.
#[test]
fn two_devices_running_one_rule_make_one_task() {
    crate::config::stand_ins_only();
    let base = scratch("once");
    let (desk, phone) = (Device::new(&base, "desk"), Device::new(&base, "phone"));
    let at_desk = desk.store(1, "Shop <shop@shop.example.org>", GENUINE, "[New sale] Gift card 50 €", "2026-11-20", &[]);
    let at_phone = phone.store(1, "Shop <shop@shop.example.org>", GENUINE, "[New sale] Gift card 50 €", "2026-11-20", &[]);
    let first = desk.arrive(&[sale_task()], std::slice::from_ref(&at_desk), false);
    assert!(first.problems.is_empty() && first.refused.is_empty(), "{first:?}");
    assert_eq!(first.done.len(), 1);
    let uid = task_uid("r-task", "mid:sale-1@shop.example.org");
    assert_eq!(first.done[0].act.result, crate::links::task_uri(&uid));
    let text = std::fs::read_to_string(desk.list.dir.join(format!("{uid}.ics"))).unwrap();
    let task = crate::tasks::task_of_text(&text, &jiff::tz::TimeZone::UTC).unwrap();
    assert_eq!((task.title.as_str(), task.due.as_str(), task.estimate), ("Send: Gift card 50 €", "2026-11-25", 15));
    assert!(text.contains("X-SIOUL-RULE:r-task") && text.contains("X-SIOUL-ORIGIN:mid:sale-1@shop.example.org") && !text.contains("To Ann"), "{text}");
    assert!(text.contains("CREATED:20261120T100000Z"), "stamped from the message's arrival: {text}");
    assert_eq!(first.done[0].said.files.len(), 1, "its Undo knows the file");
    assert!(desk.ties().links.iter().any(|e| e.how == "made" && e.from == crate::links::task_uri(&uid)));
    // The phone, the desk's acts not here yet: the same UID, the same bytes.
    let second = phone.arrive(&[sale_task()], std::slice::from_ref(&at_phone), false);
    assert_eq!(second.done.iter().map(|d| d.act.result.clone()).collect::<Vec<_>>(), [crate::links::task_uri(&uid)]);
    assert_eq!(std::fs::read_to_string(phone.list.dir.join(format!("{uid}.ics"))).unwrap(), text, "the same bytes on both devices");
    assert_eq!(second.done[0].act.digest, first.done[0].act.digest);
    // The same list (the server's copy came): found there, nothing written.
    std::fs::remove_dir_all(&phone.acts).unwrap();
    let found = phone.arrive(&[sale_task()], std::slice::from_ref(&at_phone), false);
    assert_eq!(found.done.iter().map(|d| d.act.wrote).collect::<Vec<_>>(), [false]);
    assert_eq!(phone.tasks().len(), 1);
    // The desk's record travelled to a third device: nothing at all.
    let tablet = Device { acts: desk.acts.clone(), ..Device::new(&base, "tablet") };
    let at_tablet = tablet.store(1, "Shop <shop@shop.example.org>", GENUINE, "[New sale] Gift card 50 €", "2026-11-20", &[]);
    let none = tablet.arrive(&[sale_task()], &[at_tablet], false);
    assert!(none.done.is_empty() && tablet.tasks().is_empty(), "{none:?}");
    let _ = std::fs::remove_dir_all(&base);
}

/// Undo is a state: undone on the desk before its record reached the
/// phone, which made the same task meanwhile; when the record comes, the
/// phone takes its own copy back (the same bytes: its Undo matches), and
/// the same message fetched again makes nothing on either device. A task
/// changed since stays, and is still never made again; one moved to another
/// list is found there.
#[test]
fn an_act_undone_is_taken_back_on_every_device() {
    crate::config::stand_ins_only();
    let base = scratch("undone");
    let (desk, phone) = (Device::new(&base, "desk"), Device::new(&base, "phone"));
    let mail = desk.store(1, "Shop <shop@shop.example.org>", GENUINE, "[New sale] Gift card", "2026-11-20", &[]);
    let at_phone = phone.store(1, "Shop <shop@shop.example.org>", GENUINE, "[New sale] Gift card", "2026-11-20", &[]);
    let act = desk.arrive(&[sale_task()], std::slice::from_ref(&mail), false).done[0].act.clone();
    assert_eq!(phone.arrive(&[sale_task()], std::slice::from_ref(&at_phone), false).done.len(), 1, "the phone, before the record travelled");
    let list = desk.list.clone();
    let undone = undo(&act, &desk.place(), Some(&desk.places()), &move |_| Some(list.clone()), 1_796_000_000_000).unwrap();
    assert!(undone.put_back);
    assert!(desk.tasks().is_empty() && !desk.ties().links.iter().any(|e| e.how == "made"), "the task and its tie gone");
    // The desk's record comes to the phone: its own copy taken back, at its next pass.
    phone.receive_acts(&desk);
    let settled = phone.engine(&[sale_task()], Scanned::Clean).settle(&Translator::new("en"));
    assert!(settled.problems.is_empty(), "{settled:?}");
    assert!(phone.tasks().is_empty(), "taken back on the phone");
    assert!(!phone.ties().links.iter().any(|e| e.how == "made"));
    assert!(phone.arrive(&[sale_task()], &[at_phone], false).done.is_empty());
    assert!(desk.arrive(&[sale_task()], &[mail], false).done.is_empty());
    // Changed since: it stays.
    let other = Device::new(&base, "other");
    let mail = other.store(2, "Shop <shop@shop.example.org>", GENUINE, "[New sale] Card", "2026-11-20", &[]);
    let act = other.arrive(&[sale_task()], &[mail], false).done[0].act.clone();
    let path = other.tasks()[0].clone();
    std::fs::write(&path, std::fs::read_to_string(&path).unwrap().replace("SUMMARY:", "SUMMARY:Mine ")).unwrap();
    let list = other.list.clone();
    let undone = undo(&act, &other.place(), Some(&other.places()), &move |_| Some(list.clone()), 1).unwrap();
    assert!(!undone.put_back && path.exists());
    // Moved to another list: found there and taken out.
    let moved = Device::new(&base, "moved");
    let mail = moved.store(3, "Shop <shop@shop.example.org>", GENUINE, "[New sale] Moved", "2026-11-20", &[]);
    let act = moved.arrive(&[sale_task()], &[mail], false).done[0].act.clone();
    let home = moved.dir.join("home");
    std::fs::create_dir_all(&home).unwrap();
    let file = moved.tasks()[0].clone();
    let elsewhere = home.join(file.file_name().unwrap());
    std::fs::rename(&file, &elsewhere).unwrap();
    super::acts::append(&moved.place().log(), &[Act { at: 9, rule: act.rule.clone(), object: act.object.clone(), undone: true, ..Act::default() }]).unwrap();
    let mut engine = moved.engine(&[sale_task()], Scanned::Clean);
    let gone = Collection { dir: home.clone(), id: "home".into(), ..moved.list.clone() };
    engine.all_lists = Box::new(move || vec![gone.clone()]);
    engine.find_list = Box::new(|_| None);
    assert_eq!(engine.take_back().unwrap().len(), 1);
    assert!(!elsewhere.exists(), "found in the list it moved to");
    let _ = std::fs::remove_dir_all(&base);
}

/// The guards: nothing at an address's first fetch; nothing on what came
/// before the rule, nor on mail more than a week old; nothing on mail the
/// Porch sets aside (forged), holds (a stranger in the screener), cannot
/// prove (SPF and DKIM failing: not even by its subject), nor on your own.
#[test]
fn a_rule_never_acts_where_the_guards_say_no() {
    crate::config::stand_ins_only();
    let base = scratch("guards");
    let desk = Device::new(&base, "desk");
    let genuine = desk.store(1, "Shop <shop@shop.example.org>", GENUINE, "[New sale] One", "2026-11-20", &[]);
    assert!(desk.arrive(&[sale_task()], std::slice::from_ref(&genuine), true).done.is_empty(), "a first fetch");
    let later = Rule { since: 1_900_000_000, ..sale_task() };
    assert!(desk.arrive(&[later], std::slice::from_ref(&genuine), false).done.is_empty(), "before the rule");
    let old = desk.store(5, "Shop <shop@shop.example.org>", GENUINE, "[New sale] Old", "2026-10-01", &[]);
    assert!(desk.arrive(&[sale_task()], &[old], false).done.is_empty(), "more than a week old");
    let forged = desk.store(2, "Shop <shop@shop.example.org>", FORGED, "[New sale] Two", "2026-11-20", &[]);
    let unproven = desk.store(3, "Shop <shop@shop.example.org>", FAILED, "[New sale] Three", "2026-11-20", &[]);
    let own = desk.store(4, "Me <you@you.example.org>", "", "[New sale] Four", "2026-11-20", &[]);
    let stranger = desk.store(6, "Someone <news@other.example>", STRANGER, "[New sale] Six", "2026-11-20", &[]);
    let by_subject = rule("r-subject", vec![condition("subject", "has-words", "New sale")], "task", "make", &[("model", Param::Text("send".into()))]);
    let done = desk.arrive(&[sale_task(), by_subject], &[forged, unproven, own, stranger], false);
    assert!(done.done.is_empty() && desk.tasks().is_empty(), "set aside, unproven, your own, a stranger held: {done:?}");
    let _ = std::fs::remove_dir_all(&base);
}

/// A chain stops: a rule never acts on what it made itself, nor past three
/// steps from the first object.
#[test]
fn the_chain_stops() {
    crate::config::stand_ins_only();
    let base = scratch("chain");
    let desk = Device::new(&base, "desk");
    let mail = desk.store(1, "Shop <shop@shop.example.org>", GENUINE, "[New sale] One", "2026-11-20", &[]);
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
/// nothing: no task, no tie, no act, no topic's word, no paper, no scan.
#[test]
fn the_preview_does_nothing() {
    crate::config::stand_ins_only();
    let base = scratch("preview");
    let desk = Device::new(&base, "desk");
    let files = [desk.store(1, "Shop <shop@shop.example.org>", GENUINE, "[New sale] One", "2026-11-20", &[("bill.pdf", "application/pdf", PDF)]), desk.store(2, "Shop <shop@shop.example.org>", GENUINE, "Weekly news", "2026-11-21", &[])];
    let tr = Translator::new("en");
    for then in [
        sale_task(),
        rule("r-tie", vec![condition("sender-domain", "is", "example.org")], "message", "tie-to-project", &[("project", Param::Text("gift-cards".into()))]),
        rule("r-topic", vec![condition("sender-domain", "is", "example.org")], "message", "give-topic", &[("topic", Param::Text("shop".into()))]),
        keep_papers(&[]),
    ] {
        let engine = desk.engine(&[], Scanned::Infected("never asked".into()));
        let would = engine.preview(&then, &engine.judge(&files), &tr);
        assert!(!would.is_empty(), "{}", then.id);
        assert!(would.iter().all(|w| !w.done_already && w.refused.is_none()), "{would:?}");
    }
    assert!(desk.tasks().is_empty() && !desk.dir.join("links.toml").exists() && !desk.acts.exists() && !desk.dir.join("topics").exists() && !desk.dir.join("papers").exists() && !desk.dir.join(crate::papers::MANIFEST).exists() && !desk.dir.join("scratch").exists());
    let _ = std::fs::remove_dir_all(&base);
}

/// Tie it to a project, give it a topic, tell: each once; the tie the
/// routes write; your own word on a topic stands; a notice only from the
/// device that keeps the notices (another leaves a pending line, told once
/// the lease moves there), never beside a code's own notice.
#[test]
fn a_tie_a_topic_and_a_notice() {
    crate::config::stand_ins_only();
    let base = scratch("acts");
    let desk = Device::new(&base, "desk");
    let mail = desk.store(1, "Shop <shop@shop.example.org>", GENUINE, "Your order 4 is ready", "2026-11-20", &[]);
    let code = desk.store(2, "Shop <shop@shop.example.org>", GENUINE, "Your code is 482913", "2026-11-20", &[]);
    let tie = rule("r-tie", vec![condition("sender-domain", "is", "example.org")], "message", "tie-to-project", &[("project", Param::Text("gift-cards".into()))]);
    let topic = rule("r-topic", vec![condition("sender-domain", "is", "example.org")], "message", "give-topic", &[("topic", Param::Text("shop".into()))]);
    let tell = rule("r-tell", vec![condition("sender-domain", "is", "example.org")], "none", "tell", &[("level", Param::Text("now".into()))]);
    let mut engine = desk.engine(&[tie.clone(), topic.clone(), tell.clone()], Scanned::Clean);
    engine.tells_here = true;
    let out = engine.on_arrival(&engine.judge(&[mail.clone(), code]), false, &Translator::new("en"));
    let acts: Vec<(&str, &str)> = out.done.iter().map(|d| (d.act.object.as_str(), d.act.act.as_str())).collect();
    assert!(acts.contains(&("mid:sale-1@shop.example.org", "tell")) && !acts.contains(&("mid:sale-2@shop.example.org", "tell")), "{acts:?}");
    assert!(out.refused.iter().any(|r| r.why == "rules-refused-code-told" && !r.waits), "a code has its own notice: {:?}", out.refused);
    assert!(desk.ties().links.iter().any(|e| e.how == "project" && e.from == "mid:sale-1@shop.example.org" && e.to == "sioul:project/gift-cards"));
    let said = crate::topics::labels::Said::of(&crate::topics::labels::read_all_in(&desk.dir.join("topics")));
    assert_eq!(said.message("sale-1@shop.example.org"), Some(Some("shop")));
    assert_eq!(out.notices.len(), 1);
    assert!(out.notices[0].body.contains("Your order 4 is ready"), "{}", out.notices[0].body);
    // Undo of the topic: your word taken back.
    let act = out.done.iter().find(|d| d.act.act == "topic" && d.act.object == "mid:sale-1@shop.example.org").unwrap().act.clone();
    undo(&act, &desk.place(), None, &|_| None, 1_796_000_000_000).unwrap();
    assert_eq!(crate::topics::labels::Said::of(&crate::topics::labels::read_all_in(&desk.dir.join("topics"))).message("sale-1@shop.example.org"), None);
    // Another device that does not keep the notices tells nothing: a pending line, told where the lease moves.
    let phone = Device::new(&base, "phone");
    let at_phone = phone.store(1, "Shop <shop@shop.example.org>", GENUINE, "Your order 4 is ready", "2026-11-20", &[]);
    let quiet = phone.arrive(std::slice::from_ref(&tell), std::slice::from_ref(&at_phone), false);
    assert!(quiet.notices.is_empty() && quiet.done.is_empty() && quiet.refused.iter().all(|r| r.why == "rules-refused-elsewhere" && r.waits), "{quiet:?}");
    assert!(Acts::read(&phone.acts).is_pending("r-tell", "mid:sale-1@shop.example.org"));
    let mut keeper = phone.engine(std::slice::from_ref(&tell), Scanned::Clean);
    keeper.tells_here = true;
    let told = keeper.settle(&Translator::new("en"));
    assert_eq!(told.notices.len(), 1, "the lease moved here: told now");
    // Your own word on the topic stands.
    crate::topics::labels::append(&crate::topics::labels::own_log(&phone.dir.join("topics"), "phone"), &crate::topics::labels::tagging(&["sale-1@shop.example.org".to_string()], None, 1)).unwrap();
    let yours = phone.arrive(&[topic], &[at_phone], false);
    assert_eq!(yours.refused.iter().map(|r| r.why).collect::<Vec<_>>(), ["rules-refused-your-word"]);
    let _ = std::fs::remove_dir_all(&base);
}

/// What a rule makes is kept from agents as its message is: a task from a
/// general model goes in the message's project (closed here) unless the
/// rule names one; a message holding a code gives its title nothing of its
/// subject.
#[test]
fn what_a_rule_makes_stays_in_its_message_s_project() {
    crate::config::stand_ins_only();
    let base = scratch("closed");
    let mut desk = Device::new(&base, "desk");
    desk.routes = vec![Route { from_domains: vec!["shop.example.org".into()], ..Route::default() }];
    let mail = desk.store(1, "Shop <shop@shop.example.org>", GENUINE, "[New sale] Gift card", "2026-11-20", &[]);
    let code = desk.store(2, "Shop <shop@shop.example.org>", GENUINE, "[New sale] Your code is 482913", "2026-11-20", &[]);
    let by_subject = rule("r-task", vec![condition("subject", "has-words", "New sale")], "task", "make", &[("model", Param::Text("send".into()))]);
    let out = desk.arrive(&[by_subject], &[mail, code], false);
    assert_eq!(out.done.len(), 2, "{out:?}");
    for path in desk.tasks() {
        let task = crate::tasks::task_of_text(&std::fs::read_to_string(&path).unwrap(), &jiff::tz::TimeZone::UTC).unwrap();
        assert_eq!(task.projects, ["secret"], "in its message's project, closed to agents");
        assert!(!task.title.contains("482913"), "{}", task.title);
    }
    let _ = std::fs::remove_dir_all(&base);
}

/// Mail arriving is tied to the project whose lane the Porch places it in
/// (`arrival::project_ties`, the first of `arrival::arrived`'s three steps),
/// once: a tie held already is not added again; a stranger's mail, held in
/// the screener, is tied to none.
#[test]
fn arriving_mail_is_tied_to_its_project_once() {
    crate::config::stand_ins_only();
    let base = scratch("arrival-ties");
    let mut desk = Device::new(&base, "desk");
    desk.routes = vec![Route { from_domains: vec!["shop.example.org".into()], ..Route::default() }];
    let routed = desk.store(1, "Shop <shop@shop.example.org>", GENUINE, "Your bill", "2026-11-20", &[]);
    let stranger = desk.store(2, "Someone <news@other.example>", STRANGER, "News", "2026-11-20", &[]);
    let judged = desk.engine(&[], Scanned::Clean).judge(&[routed, stranger]);
    let ties = crate::wiring::arrival::project_ties(&judged, &LocalLinks::default());
    let said: Vec<(&str, &str, &str)> = ties.iter().map(|e| (e.from.as_str(), e.to.as_str(), e.how.as_str())).collect();
    assert_eq!(said, [(crate::links::mail_uri("sale-1@shop.example.org").as_str(), crate::links::project_uri("secret").as_str(), "project")]);
    assert!(crate::wiring::arrival::project_ties(&judged, &LocalLinks { links: ties }).is_empty(), "held already");
    let _ = std::fs::remove_dir_all(&base);
}

/// A refusal that may pass waits: no list to write in, a pending line, said
/// once; once a list is there, the next pass makes the task.
#[test]
fn a_refusal_waits_and_is_tried_again() {
    crate::config::stand_ins_only();
    let base = scratch("pending");
    let desk = Device::new(&base, "desk");
    let mail = desk.store(1, "Shop <shop@shop.example.org>", GENUINE, "[New sale] Gift card", "2026-11-20", &[]);
    let mut engine = desk.engine(&[sale_task()], Scanned::Clean);
    engine.lists = Box::new(|_| None);
    let out = engine.on_arrival(&engine.judge(std::slice::from_ref(&mail)), false, &Translator::new("en"));
    assert_eq!(out.refused.iter().map(|r| (r.why, r.waits)).collect::<Vec<_>>(), [("rules-refused-no-list", true)]);
    assert!(Acts::read(&desk.acts).is_pending("r-task", "mid:sale-1@shop.example.org"));
    let again = desk.engine(&[sale_task()], Scanned::Clean).settle(&Translator::new("en"));
    assert_eq!(again.done.len(), 1, "{again:?}");
    assert_eq!(desk.tasks().len(), 1);
    let _ = std::fs::remove_dir_all(&base);
}

/// Attachments kept as papers: the PDF, named by the rule's pattern, its
/// file named with its digest, tied to its message; once across two devices
/// (the second finds the paper the wallet brought, and ties it); a file kept
/// by hand already gets the tie; two files of one name are two papers with
/// their own bytes; a page under a PDF's name, or any page, is not kept;
/// Undo takes the paper and its file out, unless the paper was filled in
/// since; the message is never changed.
#[test]
fn attachments_are_kept_as_papers_once() {
    crate::config::stand_ins_only();
    let base = scratch("papers");
    let desk = Device::new(&base, "desk");
    let mail = desk.store(1, "Shop <shop@shop.example.org>", GENUINE, "Your bill", "2026-11-20", &[("bill_november.pdf", "application/pdf", PDF), ("fake.pdf", "application/pdf", HTML), ("page.html", "text/html", HTML)]);
    let message = std::fs::read(&mail).unwrap();
    let keep = keep_papers(&[("title", Param::Text("{sender} bill, {month}".into())), ("kind", Param::Text("invoice".into())), ("type", Param::Text("any".into()))]);
    let out = desk.arrive(std::slice::from_ref(&keep), std::slice::from_ref(&mail), false);
    assert_eq!(out.done.len(), 1, "the PDF alone: {out:?}");
    let act = out.done[0].act.clone();
    assert!(act.wrote);
    let wallet = desk.wallet();
    assert_eq!(wallet.papers.len(), 1);
    let paper = &wallet.papers[0];
    assert_eq!((paper.title.as_str(), paper.unchecked), ("Shop bill, November 2026", false));
    let file = wallet.file_path(paper).unwrap();
    let name = file.file_name().unwrap().to_string_lossy().into_owned();
    assert!(name.starts_with("bill_november-") && name.ends_with(".pdf") && name.len() == "bill_november-".len() + 8 + 4, "{name}");
    assert!(std::fs::read(&file).unwrap().starts_with(b"%PDF-1.4"));
    assert!(desk.ties().links.iter().any(|e| e.how == "made" && e.from == crate::links::paper_uri(&paper.id) && e.to == "mid:sale-1@shop.example.org"));
    assert_eq!(std::fs::read(&mail).unwrap(), message, "the message never changes");
    // A second device, the wallet and its folder come with the sharing, its acts not yet.
    let phone = Device::new(&base, "phone");
    std::fs::create_dir_all(phone.dir.join("papers")).unwrap();
    std::fs::copy(desk.dir.join(crate::papers::MANIFEST), phone.dir.join(crate::papers::MANIFEST)).unwrap();
    std::fs::copy(&file, phone.dir.join("papers").join(&name)).unwrap();
    let at_phone = phone.store(1, "Shop <shop@shop.example.org>", GENUINE, "Your bill", "2026-11-20", &[("bill_november.pdf", "application/pdf", PDF)]);
    let again = phone.arrive(std::slice::from_ref(&keep), std::slice::from_ref(&at_phone), false);
    assert_eq!(again.done.iter().map(|d| d.act.wrote).collect::<Vec<_>>(), [false], "{again:?}");
    assert_eq!((phone.wallet().papers.len(), std::fs::read_dir(phone.dir.join("papers")).unwrap().count()), (1, 1));
    // A file kept by hand already: the rule ties it, keeps nothing more.
    let tablet = Device::new(&base, "tablet");
    std::fs::create_dir_all(tablet.dir.join("papers")).unwrap();
    std::fs::copy(&file, tablet.dir.join("papers").join("mine.pdf")).unwrap();
    crate::papers::Wallet::change_at(&tablet.places(), |w| w.put(crate::papers::Paper { id: "mine".into(), title: "Mine".into(), file: "papers/mine.pdf".into(), ..crate::papers::Paper::default() })).unwrap();
    let at_tablet = tablet.store(1, "Shop <shop@shop.example.org>", GENUINE, "Your bill", "2026-11-20", &[("bill_november.pdf", "application/pdf", PDF)]);
    let by_hand = tablet.arrive(std::slice::from_ref(&keep), &[at_tablet], false);
    assert_eq!(by_hand.done.iter().map(|d| (d.act.result.as_str(), d.act.wrote)).collect::<Vec<_>>(), [("sioul:paper/mine", false)]);
    // Two files of one name: two papers, each its own bytes.
    let twice = Device::new(&base, "twice");
    let both = twice.store(1, "Shop <shop@shop.example.org>", GENUINE, "Two bills", "2026-11-20", &[("bill.pdf", "application/pdf", PDF), ("bill.pdf", "application/pdf", OTHER_PDF)]);
    let two = twice.arrive(&[keep_papers(&[])], &[both], false);
    assert_eq!(two.done.len(), 2);
    let wallet = twice.wallet();
    let bytes: Vec<Vec<u8>> = wallet.papers.iter().map(|p| std::fs::read(wallet.file_path(p).unwrap()).unwrap()).collect();
    assert_eq!(bytes.len(), 2);
    assert_ne!(bytes[0], bytes[1], "each paper its own file");
    // Filled in since (an end date): yours now, Undo leaves it and its file.
    let filled = two.done[0].act.clone();
    let id = crate::links::id_of(&filled.result);
    crate::papers::Wallet::change_at(&twice.places(), |w| {
        let mut paper = w.get(&id).cloned().unwrap();
        paper.until = Some(jiff::civil::date(2027, 11, 20));
        w.put(paper);
    })
    .unwrap();
    assert!(!undo(&filled, &twice.place(), Some(&twice.places()), &|_| None, 3).unwrap().put_back);
    assert_eq!((twice.wallet().papers.len(), std::fs::read_dir(twice.dir.join("papers")).unwrap().count()), (2, 2));
    // Undo: the paper and its file go back out.
    let undone = undo(&act, &desk.place(), Some(&desk.places()), &|_| None, 3).unwrap();
    assert!(undone.put_back);
    assert!(desk.wallet().papers.is_empty() && !file.exists());
    let _ = std::fs::remove_dir_all(&base);
}

/// The antivirus: a phone, which can ask none, keeps the file marked
/// unchecked; a computer with one reads it when the wallet comes, and takes
/// it out on every device when it refuses it (said once), or takes the mark
/// off when clean; a refusal at arrival keeps nothing and says "not for
/// this one" for good, and takes out the paper a phone kept of those bytes.
#[test]
fn a_phone_keeps_unchecked_and_a_computer_refuses() {
    crate::config::stand_ins_only();
    let base = scratch("antivirus");
    let phone = Device::new(&base, "phone");
    let mail = phone.store(1, "Shop <shop@shop.example.org>", GENUINE, "Your bill", "2026-11-20", &[("bill.pdf", "application/pdf", PDF)]);
    let out = phone.arrive_scanned(&[keep_papers(&[])], std::slice::from_ref(&mail), false, Scanned::NoAntivirus);
    assert_eq!(out.done.len(), 1);
    let wallet = phone.wallet();
    assert!(wallet.papers[0].unchecked, "kept, marked unchecked");
    let file = wallet.file_path(&wallet.papers[0]).unwrap();
    // A computer, the wallet and the file come: its antivirus refuses it.
    let desk = Device::new(&base, "desk");
    std::fs::create_dir_all(desk.dir.join("papers")).unwrap();
    std::fs::copy(phone.dir.join(crate::papers::MANIFEST), desk.dir.join(crate::papers::MANIFEST)).unwrap();
    std::fs::copy(&file, desk.dir.join("papers").join(file.file_name().unwrap())).unwrap();
    let refused = desk.engine(&[], Scanned::Infected("Eicar".into())).settle(&Translator::new("en"));
    assert_eq!(refused.taken_out, ["bill"], "said once, by its title");
    assert!(desk.wallet().papers.is_empty() && std::fs::read_dir(desk.dir.join("papers")).unwrap().count() == 0, "taken out with its file");
    // Clean on another computer: the mark taken off.
    let laptop = Device::new(&base, "laptop");
    std::fs::create_dir_all(laptop.dir.join("papers")).unwrap();
    std::fs::copy(phone.dir.join(crate::papers::MANIFEST), laptop.dir.join(crate::papers::MANIFEST)).unwrap();
    std::fs::copy(&file, laptop.dir.join("papers").join(file.file_name().unwrap())).unwrap();
    laptop.engine(&[], Scanned::Clean).settle(&Translator::new("en"));
    assert!(!laptop.wallet().papers[0].unchecked);
    // At arrival, refused: nothing kept, the paper kept unchecked of those bytes taken out, never again.
    let at_phone = phone.store(2, "Shop <shop@shop.example.org>", GENUINE, "Your bill again", "2026-11-20", &[("bill.pdf", "application/pdf", PDF)]);
    let out = phone.arrive_scanned(&[keep_papers(&[])], std::slice::from_ref(&at_phone), false, Scanned::Infected("Eicar".into()));
    assert!(out.refused.iter().any(|r| r.why == "rules-refused-infected" && !r.waits), "{out:?}");
    assert!(phone.wallet().papers.is_empty(), "the unchecked paper of those bytes taken out");
    assert!(Acts::read(&phone.acts).undone("r-paper", "mid:sale-2@shop.example.org"));
    assert!(phone.arrive_scanned(&[keep_papers(&[])], &[at_phone], false, Scanned::NoAntivirus).done.is_empty(), "never kept after");
    let _ = std::fs::remove_dir_all(&base);
}

/// Names from mail made safe and unique by their content: no path, no
/// device's name, no leading dot, no character Windows refuses, a stem of
/// 60 characters at most, the extension in lower case, the digest's first
/// eight characters after the stem; types read from the bytes.
#[test]
fn names_from_mail_are_made_safe() {
    let sum = "9e1c03ab77";
    assert_eq!(kept_name("bill.PDF", sum), "bill-9e1c03ab.pdf");
    assert_eq!(kept_name("../../etc/passwd.pdf", sum), "passwd-9e1c03ab.pdf");
    assert_eq!(kept_name("..\\..\\boot.ini", sum), "boot-9e1c03ab.ini");
    assert_eq!(kept_name("CON.pdf", sum), "CON-9e1c03ab.pdf");
    assert_eq!(kept_name(".hidden.pdf", sum), "hidden-9e1c03ab.pdf");
    assert_eq!(kept_name("a:b*c?.pdf", sum), "a-b-c--9e1c03ab.pdf");
    assert_eq!(kept_name("", sum), "paper-9e1c03ab");
    let long = kept_name(&format!("{}.pdf", "x".repeat(300)), sum);
    assert_eq!(long.len(), 60 + 1 + 8 + 4, "{long}");
    assert!(content_agrees("pdf", b"%PDF-1.7\n") && !content_agrees("pdf", b"<html>"));
    assert!(!content_agrees("image", b"<svg xmlns=\"\"/>") && content_agrees("image", b"\x89PNG\r\n"));
}

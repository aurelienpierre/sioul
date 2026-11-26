// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Routes that make tasks: the title, the date asked, once per message on
//! every device, the Porch's gate, a task deleted that stays deleted, the
//! supplies planned on it, and the mail already here (the model and the
//! route in the projects' file: `projects`' tests). Invented names only: a
//! shop at shop.example.org, `[New sale]`.

use super::*;
use crate::config::{Priority, Source, TimeOff};
use crate::maildir;
use crate::porch::{SenderList, Senders};
use crate::supplies::{Qty, Use};
use crate::vdir::{Collection, Kind};

fn day(text: &str) -> Date {
    text.parse().unwrap()
}

/// An empty folder of its own for a test, under the system's temporary folder.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("sioul-routetasks-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn hours(days: &[&str], kind: &str) -> Vec<AdminWindow> {
    days.iter().map(|d| AdminWindow { day: d.to_string(), start: "09:00".into(), end: Some("17:00".into()), minutes: 0, kind: Some(kind.into()) }).collect()
}

/// The title: the route's words of the subject and the brackets around them
/// taken off, case and accents aside; separators left at an end too; the
/// prefix in front; nothing left, the subject; no subject, the model's name.
#[test]
fn a_task_s_title_is_the_subject_without_the_route_s_words() {
    let words = |list: &[&str]| list.iter().map(|w| w.to_string()).collect::<Vec<String>>();
    let sale = words(&["New sale"]);
    assert_eq!(title("[New sale] Gift card 50 €", &sale, "", "Send a gift card"), "Gift card 50 €");
    assert_eq!(title("[New sale] Gift card 50 €", &sale, "Send:", "Send a gift card"), "Send: Gift card 50 €");
    assert_eq!(title("[NEW SALE]  Gift card", &sale, "", ""), "Gift card", "case aside, spaces made one");
    assert_eq!(title("New sale: Gift card", &sale, "", ""), "Gift card", "no brackets: the separator left goes");
    assert_eq!(title("Gift card – New sale", &sale, "", ""), "Gift card");
    assert_eq!(title("[Nouvelle vente] Carte cadeau", &words(&["nouvelle vente"]), "", ""), "Carte cadeau");
    assert_eq!(title("[Vente réalisée] Carte", &words(&["vente realisee"]), "", ""), "Carte", "accents aside");
    assert_eq!(title("[New sale #12] Gift card", &sale, "", ""), "[#12] Gift card", "brackets that hold more stay, tidied");
    assert_eq!(title("Re: [New sale] Gift card", &sale, "", ""), "Re: Gift card");
    assert_eq!(title("[New sale]", &sale, "Send:", "Send a gift card"), "Send: [New sale]", "nothing left: the subject");
    assert_eq!(title("", &sale, "", "Send a gift card"), "Send a gift card", "no subject: the model's name");
    assert_eq!(title("Newsale Gift card", &sale, "", ""), "Newsale Gift card", "whole words only");
    assert_eq!(bracketed("[New sale] Gift card [Shop] 【Paid】"), ["New sale", "Shop", "Paid"]);
    assert!(bracketed("Gift card").is_empty());
}

/// The date asked: so many business days after the mail came, counted from
/// the day after (0: the first business day on or after it); over a
/// weekend; over time off typed and a public holiday of a calendar that
/// counts (one that does not count is a working day); the days with hours
/// of work in the week, Monday to Friday when none is set, admin's hours
/// alone being none.
#[test]
fn the_date_asked_counts_business_days() {
    let none = Workdays::new(&[], DaysOff::default());
    // Friday 20 November 2026.
    assert_eq!(none.date_asked(day("2026-11-20"), 3), day("2026-11-25"), "over the weekend: Monday, Tuesday, Wednesday");
    assert_eq!(none.date_asked(day("2026-11-20"), 0), day("2026-11-20"), "0: the day it came, a business day");
    assert_eq!(none.date_asked(day("2026-11-21"), 0), day("2026-11-23"), "0 on a Saturday: Monday");
    assert_eq!(none.date_asked(day("2026-11-21"), 1), day("2026-11-23"), "1 from a Saturday: Monday");
    assert_eq!(none.date_asked(day("2026-11-20"), 1), day("2026-11-23"));
    // Time off typed: Monday and Tuesday.
    let typed = vec![TimeOff { id: "a".into(), from: day("2026-11-23"), until: day("2026-11-24"), label: String::new(), kind: "vacation".into() }];
    let away = Workdays::new(&[], DaysOff::typed(&typed));
    assert_eq!(away.date_asked(day("2026-11-20"), 3), day("2026-11-27"));
    // A public holiday, Friday 25 December 2026, its calendar counting as days off.
    let public = crate::subscribed::HolidayDays { id: "fr".into(), name: "Public holidays".into(), holidays: crate::subscribed::Holidays::Public, days_off: true, periods: vec![(day("2026-12-25"), day("2026-12-25"), "Christmas".into())], as_of: Some(1) };
    let holidays = Workdays::new(&[], DaysOff::with_holidays(&[], std::slice::from_ref(&public)));
    assert_eq!(holidays.date_asked(day("2026-12-24"), 1), day("2026-12-28"));
    let not_counted = Workdays::new(&[], DaysOff::with_holidays(&[], &[crate::subscribed::HolidayDays { days_off: false, ..public }]));
    assert_eq!(not_counted.date_asked(day("2026-12-24"), 1), day("2026-12-25"));
    // Work on Tuesdays and Thursdays only; admin's hours on Monday count for nothing.
    let mut week = hours(&["tuesday", "thursday"], "work");
    week.extend(hours(&["monday"], "admin"));
    let two = Workdays::new(&week, DaysOff::default());
    assert_eq!(two.date_asked(day("2026-11-23"), 2), day("2026-11-26"));
    assert_eq!(two.date_asked(day("2026-11-23"), 0), day("2026-11-24"));
    let admin_only = Workdays::new(&hours(&["saturday"], "admin"), DaysOff::default());
    assert_eq!(admin_only.date_asked(day("2026-11-20"), 1), day("2026-11-23"), "no hours of work: Monday to Friday");
    // Every day off for years: n days after, never a loop without end.
    let always = vec![TimeOff { id: "b".into(), from: day("2026-01-01"), until: day("2099-12-31"), label: String::new(), kind: "sick".into() }];
    assert_eq!(Workdays::new(&[], DaysOff::typed(&always)).date_asked(day("2026-11-20"), 2), day("2026-11-22"));
}

/// A mail folder, a gate on it as the Porch has it, a project whose route
/// makes tasks, and a task list: what the tests below share.
struct Shop {
    dir: PathBuf,
    mail: PathBuf,
    list: Collection,
    gate: Gate,
    store: ProjectStore,
    config: Config,
    stored: std::cell::Cell<u32>,
}

const GENUINE: &str = "spf=pass smtp.mailfrom=example.org; dkim=pass header.d=example.org; dmarc=pass header.from=example.org";
const FORGED: &str = "spf=fail smtp.mailfrom=attacker.example; dkim=none; dmarc=fail header.from=example.org";
const FAILED: &str = "spf=fail smtp.mailfrom=example.org; dkim=fail header.d=example.org";

impl Shop {
    fn new(name: &str, route: Route) -> Shop {
        let dir = scratch(name);
        let mail = dir.join("mail");
        std::fs::create_dir_all(mail.join("cur")).unwrap();
        std::fs::create_dir_all(dir.join("work")).unwrap();
        let list = Collection { kind: Kind::Calendars, account: "local".into(), id: "work".into(), dir: dir.join("work"), name: "Work".into(), color: None, read_only: false, components: vec!["VTODO".into()] };
        let model = Model { id: "send".into(), name: "Send a gift card".into(), list: "local/work".into(), prefix: String::new(), business_days: Some(3), estimate: Some(15), supplies: vec![Use { id: "stamps".into(), quantity: Qty::whole(1) }, Use { id: "cards".into(), quantity: Qty::whole(1) }] };
        let project = Project { id: "gift-cards".into(), title: "Gift cards".into(), routes: vec![Route { id: "r1".into(), makes: "send".into(), ..route }], models: vec![model], ..Project::default() };
        let store = ProjectStore { root: dir.clone(), notes: dir.clone(), projects: vec![project], ties: Default::default() };
        let senders = Senders { blocked: SenderList::parse("pest@example.org"), ..Senders::default() };
        let source = Source { account: Some("home".into()), address: Some("you@you.example.org".into()), folder: mail.clone(), trusted_ids: vec!["mx.provider.example".into()], priority: Priority::Average, shielded: false, words: crate::words::Words::builtin().into(), spam: None };
        let gate = Gate::new(vec![source], Some(store.clone()), SenderList::default(), senders, 1_795_000_000);
        Shop { dir, mail, list, gate, store, config: Config::default(), stored: std::cell::Cell::new(0) }
    }

    /// A message stored in the mail folder, arrived on `on` (its file's time and its Date).
    fn store(&self, from: &str, results: &str, subject: &str, on: &str) -> PathBuf {
        let n = self.stored.get() + 1;
        self.stored.set(n);
        let path = self.mail.join("cur").join(format!("{n}.U1-{n}.sioul{}2,S", maildir::INFO));
        let results = if results.is_empty() { String::new() } else { format!("Authentication-Results: mx.provider.example; {results}\r\n") };
        let at = format!("{on}T10:00").parse::<jiff::civil::DateTime>().unwrap().to_zoned(jiff::tz::TimeZone::UTC).unwrap();
        std::fs::write(&path, format!("{results}Message-ID: <sale-{n}@example.org>\r\nFrom: {from}\r\nDate: {}\r\nSubject: {subject}\r\n\r\nPrint: To Ann, 50 EUR.\r\n", at.strftime("%a, %d %b %Y %H:%M:%S %z"))).unwrap();
        let time = std::time::UNIX_EPOCH + std::time::Duration::from_secs(u64::try_from(at.timestamp().as_second()).unwrap());
        std::fs::File::options().write(true).open(&path).unwrap().set_modified(time).unwrap();
        path
    }

    /// The tasks `files` make on a device that knows the tasks `known` and the ties `ties`.
    fn arrive(&self, files: &[PathBuf], known: &dyn Fn(&str) -> bool, ties: &LocalLinks) -> Outcome {
        self.arrive_at(files, known, ties, "2026-11-23T12:00")
    }

    /// The same, the device's clock at `now`.
    fn arrive_at(&self, files: &[PathBuf], known: &dyn Fn(&str) -> bool, ties: &LocalLinks, now: &str) -> Outcome {
        let workdays = Workdays::new(&[], DaysOff::default());
        let list = self.list.clone();
        let lists = move |_: &Model| Some(list.clone());
        let zone = jiff::tz::TimeZone::UTC;
        let now = now.parse::<jiff::civil::DateTime>().unwrap().to_zoned(zone.clone()).unwrap();
        let own = vec!["you@you.example.org".to_string()];
        let making = Making { config: &self.config, own: &own, lists: &lists, workdays: &workdays, known, ties, zone: &zone, now: &now };
        on_arrival(&self.gate, &self.store, files, &making, &self.dir.join("links.toml"))
    }

    fn ties(&self) -> LocalLinks {
        LocalLinks::load(&self.dir.join("links.toml"))
    }
}

fn shop_route() -> Route {
    Route { from_addresses: vec!["shop@example.org".into()], subject_contains: vec!["New sale".into()], ..Route::default() }
}

/// One task per message, as the route makes it: its title, its project,
/// its date asked, its estimate, its supplies, where it came from; tied to
/// its message as made; its UID from the project and the Message-ID, so that
/// a second device that gets the same mail writes nothing more (the file is
/// there) and ties it the same way. No text of the mail copied.
#[test]
fn one_task_per_message_on_every_device() {
    let shop = Shop::new("once", shop_route());
    // Friday 20 November 2026.
    let sale = shop.store("Shop <shop@example.org>", GENUINE, "[New sale] Gift card 50 €", "2026-11-20");
    let nobody = |_: &str| false;
    let first = shop.arrive(std::slice::from_ref(&sale), &nobody, &LocalLinks::default());
    assert!(first.problems.is_empty(), "{:?}", first.problems);
    assert_eq!(first.made.len(), 1);
    let made = &first.made[0];
    assert_eq!(made.uid, uid("gift-cards", "sale-1@example.org"));
    assert_eq!((made.title.as_str(), made.due, made.list.as_str()), ("Gift card 50 €", day("2026-11-25"), "Work"));
    let text = std::fs::read_to_string(shop.list.dir.join(format!("{}.ics", made.uid))).unwrap();
    let task = crate::tasks::task_of_text(&text, &jiff::tz::TimeZone::UTC).unwrap();
    assert_eq!((task.title.as_str(), task.due.as_str(), task.estimate, task.projects.clone()), ("Gift card 50 €", "2026-11-25", 15, vec!["gift-cards".to_string()]));
    assert_eq!(task.origin, "mid:sale-1@example.org");
    assert_eq!(task.supplies.len(), 2);
    assert!(!text.contains("To Ann") && !text.contains("50 EUR"), "nothing of the mail's text: {text}");
    let ties = shop.ties();
    assert_eq!(ties.links, [Edge { from: links::task_uri(&made.uid), to: "mid:sale-1@example.org".into(), how: MADE.into() }]);
    // A second device, the same mail, its own ties not travelled yet: the same file, nothing new.
    let second = shop.arrive(std::slice::from_ref(&sale), &nobody, &LocalLinks::default());
    assert!(second.made.is_empty());
    assert_eq!(second.ties, ties.links, "tied the same way");
    assert_eq!(std::fs::read_dir(&shop.list.dir).unwrap().count(), 1);
    // The same message twice in one fetch (two folders): one task.
    let other = shop.store("Shop <shop@example.org>", GENUINE, "[New sale] Gift card 20 €", "2026-11-20");
    let copy = shop.mail.join("cur").join(format!("copy{}2,S", maildir::INFO));
    std::fs::copy(&other, &copy).unwrap();
    assert_eq!(shop.arrive(&[other, copy], &nobody, &shop.ties()).made.len(), 1);
    let _ = std::fs::remove_dir_all(&shop.dir);
}

/// The Porch's gate: forged mail, a blocked sender and a borrowed name make
/// no task; mail nothing authenticates makes none by a route on its sender,
/// one by a route on its subject alone; mail from yourself makes none; a
/// message the route does not take makes none.
#[test]
fn mail_the_porch_sets_aside_makes_no_task() {
    let shop = Shop::new("gate", shop_route());
    let nobody = |_: &str| false;
    let forged = shop.store("Shop <shop@example.org>", FORGED, "[New sale] Gift card", "2026-11-20");
    let unproven = shop.store("Shop <shop@example.org>", FAILED, "[New sale] Gift card", "2026-11-20");
    let other = shop.store("Shop <shop@example.org>", GENUINE, "Your weekly news", "2026-11-20");
    let made = shop.arrive(&[forged, unproven, other], &nobody, &LocalLinks::default());
    assert!(made.made.is_empty() && made.ties.is_empty(), "{made:?}");
    // A route on the subject alone takes mail nothing authenticates, as the Porch's lanes do;
    // never forged mail, a blocked sender, a borrowed name, nor your own.
    let by_subject = Shop::new("gate-subject", Route { subject_contains: vec!["New sale".into()], ..Route::default() });
    let files = [
        by_subject.store("Shop <shop@example.org>", FAILED, "[New sale] Gift card", "2026-11-20"),
        by_subject.store("Shop <shop@example.org>", FORGED, "[New sale] Gift card", "2026-11-20"),
        by_subject.store("Pest <pest@example.org>", GENUINE, "[New sale] Gift card", "2026-11-20"),
        by_subject.store("PayPal <service@example.org>", GENUINE, "[New sale] Gift card", "2026-11-20"),
        by_subject.store("Me <you@you.example.org>", "", "Re: [New sale] Gift card", "2026-11-20"),
    ];
    let made = by_subject.arrive(&files, &nobody, &LocalLinks::default()).made;
    assert_eq!(made.iter().map(|m| m.uid.clone()).collect::<Vec<_>>(), [uid("gift-cards", "sale-1@example.org")]);
    let _ = std::fs::remove_dir_all(&shop.dir);
    let _ = std::fs::remove_dir_all(&by_subject.dir);
}

/// A task the person deleted is never made again: its tie stays, here and
/// on the devices it travels to, whether the same mail comes again (a
/// device that fetches it later) or "Also for the messages already here" is
/// asked; a task moved to another list (its UID known) is not made twice.
#[test]
fn a_deleted_task_stays_deleted() {
    let shop = Shop::new("deleted", shop_route());
    let sale = shop.store("Shop <shop@example.org>", GENUINE, "[New sale] Gift card", "2026-11-20");
    let nobody = |_: &str| false;
    let made = shop.arrive(std::slice::from_ref(&sale), &nobody, &LocalLinks::default()).made;
    std::fs::remove_file(&made[0].path).unwrap();
    let again = shop.arrive(std::slice::from_ref(&sale), &nobody, &shop.ties());
    assert!(again.made.is_empty() && again.ties.is_empty());
    assert!(!made[0].path.exists());
    // Known elsewhere (moved to another list), no tie here yet: not made twice.
    let uid = made[0].uid.clone();
    let known = move |u: &str| u == uid;
    assert!(shop.arrive(std::slice::from_ref(&sale), &known, &LocalLinks::default()).made.is_empty());
    let _ = std::fs::remove_dir_all(&shop.dir);
}

/// The supplies the model plans are on the task made, and count in what is
/// free at once (`supplies::planned`): a sale arriving with eight cards in
/// stock leaves seven free before the card is used.
#[test]
fn a_made_task_s_supplies_count_in_what_is_free_at_once() {
    let shop = Shop::new("supplies", shop_route());
    let sale = shop.store("Shop <shop@example.org>", GENUINE, "[New sale] Gift card", "2026-11-20");
    let made = shop.arrive(std::slice::from_ref(&sale), &|_: &str| false, &LocalLinks::default()).made;
    let task = crate::tasks::task_of_text(&std::fs::read_to_string(&made[0].path).unwrap(), &jiff::tz::TimeZone::UTC).unwrap();
    let cards = crate::supplies::Supply { id: "cards".into(), name: "Cards".into(), low_at: Some(Qty::whole(7)), ..crate::supplies::Supply::default() };
    let count = crate::supplies::by_hand("cards", crate::supplies::Kind::Count, Qty::whole(8), 1_795_000_000);
    let latest = crate::supplies::latest(&[count]);
    let standing = crate::supplies::standing(&cards, &latest, &crate::supplies::planned(std::slice::from_ref(&task)));
    assert_eq!((standing.stock, standing.planned, standing.free, standing.low), (Qty::whole(8), Qty::whole(1), Qty::whole(7), true));
    let _ = std::fs::remove_dir_all(&shop.dir);
}

/// The mail already here: what the route takes as the Porch admits it, from
/// the mail index, none it set aside; made once when asked, then none left;
/// a route that needs the text, only among the messages tied to the project.
#[test]
fn the_mail_already_here_makes_tasks_once_when_asked() {
    let shop = Shop::new("already", shop_route());
    shop.store("Shop <shop@example.org>", GENUINE, "[New sale] Gift card 50 €", "2026-11-16");
    shop.store("Shop <shop@example.org>", FORGED, "[New sale] Gift card 20 €", "2026-11-17");
    shop.store("Shop <shop@example.org>", GENUINE, "[New sale] Gift card 30 €", "2026-11-18");
    shop.store("Shop <shop@example.org>", GENUINE, "Weekly news", "2026-11-18");
    let mut index = MailIndex::default();
    for path in std::fs::read_dir(shop.mail.join("cur")).unwrap().map(|e| e.unwrap().path()) {
        let card = maildir::read_one(&path).unwrap();
        index.insert(card.message_id.as_deref().unwrap(), MailRef { path: path.clone(), subject: card.subject.clone(), from: String::new(), address: card.from_address.clone().unwrap_or_default().to_lowercase(), date: card.date.unwrap_or(0), refs: Vec::new() });
    }
    let workdays = Workdays::new(&[], DaysOff::default());
    let list = shop.list.clone();
    let lists = move |_: &Model| Some(list.clone());
    let zone = jiff::tz::TimeZone::UTC;
    let now = "2026-11-23T12:00".parse::<jiff::civil::DateTime>().unwrap().to_zoned(zone.clone()).unwrap();
    let nobody = |_: &str| false;
    let ties = LocalLinks::default();
    let making = Making { config: &shop.config, own: &[], lists: &lists, workdays: &workdays, known: &nobody, ties: &ties, zone: &zone, now: &now };
    let here = already_here(&shop.gate, &shop.store, "gift-cards", "r1", &index, &making);
    assert_eq!(here.iter().map(|c| c.subject.as_str()).collect::<Vec<_>>(), ["[New sale] Gift card 50 €", "[New sale] Gift card 30 €"]);
    let done = for_already_here(&here, &shop.store, "gift-cards", "r1", &making, &shop.dir.join("links.toml"));
    assert_eq!(done.made.iter().map(|m| (m.title.as_str(), m.due)).collect::<Vec<_>>(), [("Gift card 50 €", day("2026-11-19")), ("Gift card 30 €", day("2026-11-23"))]);
    let ties = shop.ties();
    let making = Making { ties: &ties, ..making };
    assert!(already_here(&shop.gate, &shop.store, "gift-cards", "r1", &index, &making).is_empty(), "once");
    // A route on the text: only what is tied to the project.
    let mut by_text = shop.store.clone();
    by_text.projects[0].routes[0] = Route { id: "r1".into(), text_contains: vec!["Print".into()], makes: "send".into(), ..Route::default() };
    let none = LocalLinks::default();
    assert!(already_here(&shop.gate, &by_text, "gift-cards", "r1", &index, &Making { ties: &none, ..making }).is_empty());
    let mut tied = LocalLinks::default();
    tied.add("mid:sale-4@example.org", "sioul:project/gift-cards", "project");
    assert_eq!(already_here(&shop.gate, &by_text, "gift-cards", "r1", &index, &Making { ties: &tied, ..making }).len(), 1);
    let _ = std::fs::remove_dir_all(&shop.dir);
}

/// Two devices whose clocks differ write the same bytes for one message:
/// the task is stamped from the message's arrival, so that their servers see
/// one task, never one "changed on both sides".
#[test]
fn two_devices_write_the_same_bytes() {
    let shop = Shop::new("bytes", shop_route());
    let sale = shop.store("Shop <shop@example.org>", GENUINE, "[New sale] Gift card", "2026-11-20");
    let nobody = |_: &str| false;
    let made = shop.arrive_at(std::slice::from_ref(&sale), &nobody, &LocalLinks::default(), "2026-11-23T12:00").made;
    let first = std::fs::read_to_string(&made[0].path).unwrap();
    std::fs::remove_file(&made[0].path).unwrap();
    shop.arrive_at(&[sale], &nobody, &LocalLinks::default(), "2026-11-24T09:31").made;
    assert_eq!(std::fs::read_to_string(&made[0].path).unwrap(), first);
    let _ = std::fs::remove_dir_all(&shop.dir);
}

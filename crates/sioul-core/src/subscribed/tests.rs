// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Tests of the calendars subscribed by their address: stand-ins only, never
//! a real address. The official files' shapes are cut from what
//! audit-calendars §10 quotes, with invented days.

use super::*;

const MOODLE: &str = "BEGIN:VCALENDAR\r\nMETHOD:PUBLISH\r\nPRODID:-//Moodle Pty Ltd//NONSGML Moodle Version 2025100600//EN\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:101@school.example.org/moodle\r\nSUMMARY:Essay is due\r\nDESCRIPTION:Hand in the essay.\r\nCLASS:PUBLIC\r\nLAST-MODIFIED:20301105T081000Z\r\nDTSTAMP:@STAMP@\r\nDTSTART:20301120T160000Z\r\nDTEND:20301120T160000Z\r\nCATEGORIES:LIT-3\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nUID:102@school.example.org/moodle\r\nSUMMARY:Quiz opens\r\nCLASS:PUBLIC\r\nLAST-MODIFIED:20301105T081000Z\r\nDTSTAMP:@STAMP@\r\nDTSTART:20301122T080000Z\r\nDTEND:20301122T080000Z\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";

/// Moodle stamps every event with the time of the request: two answers a minute apart.
fn moodle(stamp: &str) -> String {
    MOODLE.replace("@STAMP@", stamp)
}

/// The Ministry's school holidays, as its static files are shaped (a full
/// VTIMEZONE, TRANSP, LOCATION, DESCRIPTION, a zero-length bridge, the
/// teachers' one-day return), the days invented.
const SCHOOL: &str = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//MENJS//Calendrier//FR\r\nCALSCALE:GREGORIAN\r\nX-WR-CALNAME:Calendrier scolaire - Zone A\r\nX-WR-CALDESC:Congés scolaires de la zone A depuis l'année 2017-2018\r\nX-WR-TIMEZONE:Europe/Paris\r\nBEGIN:VTIMEZONE\r\nTZID:Europe/Paris\r\nTZURL:http://tzurl.org/zoneinfo/Europe/Paris\r\nX-LIC-LOCATION:Europe/Paris\r\nBEGIN:DAYLIGHT\r\nTZOFFSETFROM:+0100\r\nTZOFFSETTO:+0200\r\nTZNAME:CEST\r\nDTSTART:19810329T020000\r\nRRULE:FREQ=YEARLY;BYMONTH=3;BYDAY=-1SU\r\nEND:DAYLIGHT\r\nBEGIN:STANDARD\r\nTZOFFSETFROM:+0200\r\nTZOFFSETTO:+0100\r\nTZNAME:CET\r\nDTSTART:19961027T030000\r\nRRULE:FREQ=YEARLY;BYMONTH=10;BYDAY=-1SU\r\nEND:STANDARD\r\nEND:VTIMEZONE\r\n\
BEGIN:VEVENT\r\nDTSTART;VALUE=DATE:20301019\r\nDTEND;VALUE=DATE:20301104\r\nDTSTAMP:20301001T094727Z\r\nUID:20301001T094829Z-Zone-A@data.education.gouv.fr\r\nDESCRIPTION:Vacances de la Toussaint - Zone A\r\nLOCATION:Zone A\r\nSUMMARY:Vacances de la Toussaint - Zone A\r\nTRANSP:TRANSPARENT\r\nEND:VEVENT\r\n\
BEGIN:VEVENT\r\nDTSTART;VALUE=DATE:20310509\r\nDTEND;VALUE=DATE:20310509\r\nDTSTAMP:20301001T094727Z\r\nUID:20301001T094830Z-Zone-A@data.education.gouv.fr\r\nDESCRIPTION:Pont de l'Ascension - Zone A\r\nLOCATION:Zone A\r\nSUMMARY:Pont de l'Ascension - Zone A\r\nTRANSP:TRANSPARENT\r\nEND:VEVENT\r\n\
BEGIN:VEVENT\r\nDTSTART;VALUE=DATE:20300706\r\nDTEND;VALUE=DATE:20300903\r\nDTSTAMP:20301001T094727Z\r\nUID:20301001T094831Z-Zone-A@data.education.gouv.fr\r\nDESCRIPTION:Vacances d'Été - Zone A\r\nLOCATION:Zone A\r\nSUMMARY:Vacances d'Été - Zone A\r\nTRANSP:TRANSPARENT\r\nEND:VEVENT\r\n\
BEGIN:VEVENT\r\nDTSTART;VALUE=DATE:20300902\r\nDTEND;VALUE=DATE:20300903\r\nDTSTAMP:20301001T094727Z\r\nUID:20301001T094832Z-Zone-A@data.education.gouv.fr\r\nDESCRIPTION:Vacances d'Été(prérentrée Enseignants) - Zone A\r\nLOCATION:Zone A\r\nSUMMARY:Vacances d'Été(prérentrée Enseignants) - Zone A\r\nTRANSP:TRANSPARENT\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";

/// Etalab's public holidays, as its files are shaped (no TRANSP, no
/// DESCRIPTION, a stable hashed UID, the year's first day as DTSTAMP), the days invented.
const PUBLIC: &str = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//DINUM//Jours fériés Métropole//FR\r\nX-WR-CALNAME:Jours fériés Métropole\r\nNAME:Jours fériés Métropole\r\nMETHOD:PUBLISH\r\nBEGIN:VEVENT\r\nSUMMARY:Jour de Noël\r\nDTSTART;VALUE=DATE:20301225\r\nDTEND;VALUE=DATE:20301226\r\nDTSTAMP:20300101T000000Z\r\nUID:0a1b2c3d4e5f60718293a4b5c6d7e8f9\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nSUMMARY:1er janvier\r\nDTSTART;VALUE=DATE:20310101\r\nDTEND;VALUE=DATE:20310102\r\nDTSTAMP:20300101T000000Z\r\nUID:1a1b2c3d4e5f60718293a4b5c6d7e8f9\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";

fn day(text: &str) -> Date {
    text.parse().unwrap()
}

fn subscription(url: &str, holidays: Option<&str>, after_class: Option<bool>) -> Subscription {
    Subscription { url: url.into(), holidays: holidays.map(str::to_string), after_class, ..Subscription::default() }
}

fn calendar(bytes: &str) -> Answer {
    Answer::Calendar { bytes: bytes.as_bytes().to_vec(), etag: String::new(), last_modified: String::new() }
}

/// `webcal://` reads as https; http, no host, a name and password, spaces: refused.
#[test]
fn an_address_is_read_as_https_alone() {
    assert_eq!(address("webcal://calendar.example.org/a.ics").unwrap(), "https://calendar.example.org/a.ics");
    assert_eq!(address(" WEBCALS://calendar.example.org/a.ics ").unwrap(), "https://calendar.example.org/a.ics");
    assert_eq!(address("<https://school.example.org/calendar/export_execute.php?userid=7&authtoken=abc>").unwrap(), "https://school.example.org/calendar/export_execute.php?userid=7&authtoken=abc");
    assert_eq!(address("http://calendar.example.org/a.ics"), Err(Problem::NotHttps));
    assert_eq!(address("ftp://calendar.example.org/a.ics"), Err(Problem::NotHttps));
    assert_eq!(address("https://"), Err(Problem::NotAddress));
    assert_eq!(address("https://me:secret@calendar.example.org/a.ics"), Err(Problem::NotAddress));
    assert_eq!(address("https://calendar.example.org/a b.ics"), Err(Problem::NotAddress));
    assert_eq!(address("hello"), Err(Problem::NotAddress));
    assert!(official_school(&officials().iter().find(|o| o.id == "fr-school-zone-a").unwrap().url));
    assert_eq!(officials().iter().filter(|o| o.holidays == Holidays::Public).count(), 13);
    assert_eq!(officials().iter().filter(|o| o.holidays == Holidays::School).count(), 14);
}

/// What is not a calendar replaces nothing: a web page, a line of text (the
/// server's own words kept), an empty answer, something else; too large once
/// unpacked, or too many events, neither. A calendar without events is one.
#[test]
fn what_is_not_a_calendar_is_said_and_refused() {
    assert_eq!(read(b"<!DOCTYPE html><html><body>Sign in</body></html>"), Err(Problem::Page));
    assert_eq!(read(b"\n  <html lang=\"fr\">"), Err(Problem::Page));
    assert_eq!(read(b"Invalid authentication"), Err(Problem::Text("Invalid authentication".into())));
    assert_eq!(read(b"no export\n"), Err(Problem::Text("no export".into())));
    assert_eq!(read(b"   \r\n"), Err(Problem::Empty));
    assert_eq!(read(b""), Err(Problem::Empty));
    assert_eq!(read("{\"error\": true}".repeat(40).as_bytes()), Err(Problem::NotCalendar));
    assert_eq!(read(&vec![b' '; MOST_BYTES as usize + 1]), Err(Problem::TooLarge));
    let mut many = String::from("BEGIN:VCALENDAR\r\nVERSION:2.0\r\n");
    for n in 0..=MOST_EVENTS {
        many.push_str(&format!("BEGIN:VEVENT\r\nUID:{n}\r\nDTSTART:20300101T000000Z\r\nEND:VEVENT\r\n"));
    }
    many.push_str("END:VCALENDAR\r\n");
    assert_eq!(read(many.as_bytes()), Err(Problem::TooMany));
    let empty = read(b"BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Example//EN\r\nX-WR-CALNAME:Nothing coming\r\nEND:VCALENDAR\r\n").unwrap();
    assert!(empty.items.is_empty() && empty.name == "Nothing coming");
    let feed = read(PUBLIC.as_bytes()).unwrap();
    assert_eq!((feed.items.len(), feed.name.as_str(), feed.refresh), (2, "Jours fériés Métropole", None));
    // A calendar's own wish: REFRESH-INTERVAL first, else X-PUBLISHED-TTL.
    let wishes = PUBLIC.replace("METHOD:PUBLISH\r\n", "METHOD:PUBLISH\r\nREFRESH-INTERVAL;VALUE=DURATION:PT48H\r\nX-PUBLISHED-TTL:PT1H\r\n");
    assert_eq!(read(wishes.as_bytes()).unwrap().refresh, Some(48 * 3600));
    for (text, seconds) in [("P1D", Some(86_400)), ("PT48H", Some(172_800)), ("P1W", Some(604_800)), ("PT6H30M", Some(23_400)), ("PT0S", None), ("1 day", None), ("P1H", None)] {
        assert_eq!(duration(text), seconds, "{text}");
    }
}

/// Moodle's answers differ by their stamps alone: the same steady hash, one
/// write, then nothing written; an event changed, the whole copy replaced.
/// A page then keeps the last good copy, says so and since when; the
/// server's 304 writes nothing either.
#[test]
fn unchanged_but_for_its_stamps_writes_nothing() {
    crate::config::stand_ins_only();
    let (first, second) = (read(moodle("20301118T100000Z").as_bytes()).unwrap(), read(moodle("20301118T100100Z").as_bytes()).unwrap());
    assert_eq!(first.hash, second.hash);
    assert_ne!(first.items, second.items, "the stamps differ all the same");
    let id = "moodle-test";
    let s = subscription("https://school.example.org/calendar/export_execute.php?userid=7&authtoken=0123", None, None);
    vdir::prepare_subscribed(id, "Moodle", None).unwrap();
    assert!(due(id, &s, 1_000, false), "never read here");
    assert_eq!(take(id, &s, calendar(&moodle("20301118T100000Z")), 1_000).unwrap(), Taken::Changed);
    let file = folder(id).join(&first.items[0].0);
    let written = std::fs::metadata(&file).unwrap().modified().unwrap();
    assert_eq!(take(id, &s, calendar(&moodle("20301118T100100Z")), 2_000).unwrap(), Taken::Unchanged);
    assert_eq!(std::fs::metadata(&file).unwrap().modified().unwrap(), written, "nothing written");
    assert_eq!(std::fs::read_to_string(&file).unwrap(), first.items[0].1, "the first copy's stamps stay");
    assert_eq!(Fetched::load(id).checked, 2_000);
    assert!(!due(id, &s, 2_000 + EVERY - 1, false) && due(id, &s, 2_000 + EVERY, false));
    let moved = moodle("20301118T100200Z").replace("SUMMARY:Quiz opens", "SUMMARY:Quiz opens, moved");
    assert_eq!(take(id, &s, calendar(&moved), 3_000).unwrap(), Taken::Changed);
    assert!(folder(id).read_dir().unwrap().flatten().any(|e| std::fs::read_to_string(e.path()).unwrap_or_default().contains("Quiz opens, moved")));
    // A sign-in page: the last good copy stays, said with what to do.
    let page = Answer::Calendar { bytes: b"<html><body>Log in</body></html>".to_vec(), etag: String::new(), last_modified: String::new() };
    assert_eq!(take(id, &s, page, 4_000).unwrap(), Taken::Kept(Problem::Page));
    assert_eq!(folder(id).read_dir().unwrap().flatten().filter(|e| e.path().extension().is_some_and(|x| x == "ics")).count(), 2);
    let state = Fetched::load(id);
    assert_eq!((state.problem.as_str(), state.since, state.checked), ("subscribe-page", 4_000, 3_000));
    assert_eq!(every(&s, &state), SOONEST, "after a problem, within the hour");
    let tr = crate::i18n::Translator::new("en");
    let now = jiff::Timestamp::from_second(5_000).unwrap().to_zoned(jiff::tz::TimeZone::UTC);
    let line = line(id, &now, &tr);
    assert!(line.contains("no longer gives a calendar") && line.contains("last copy stays"), "{line}");
    assert!(!line.contains("authtoken") && !line.contains("example.org"), "never the address: {line}");
    // A line of text keeps the server's words; the network's failure is said as such.
    assert_eq!(take(id, &s, Answer::Refused(Problem::Network), 6_000).unwrap(), Taken::Kept(Problem::Network));
    assert_eq!(Fetched::load(id).since, 6_000, "another problem, another since");
    // The server says unchanged (304): nothing written, found current.
    assert_eq!(take(id, &s, Answer::Unchanged, 7_000).unwrap(), Taken::Unchanged);
    assert_eq!(Fetched::load(id).problem, "");
    // Another address: read whole again, whatever its hash.
    let other = Subscription { url: "https://school.example.org/other.ics".into(), ..s.clone() };
    assert_eq!(validators(id, &other), None);
    assert!(due(id, &other, 7_001, false));
    assert_eq!(take(id, &other, calendar(&moved), 8_000).unwrap(), Taken::Changed);
    vdir::remove_subscribed(id).unwrap();
}

/// How often: once a day for holidays, three hours for the rest, the
/// calendar's wish between one hour and one day; when Sioul opens, unless
/// read in the last ten minutes.
#[test]
fn each_calendar_is_read_at_its_time() {
    let plain = subscription("https://calendar.example.org/a.ics", None, None);
    let holidays = subscription("https://calendar.example.org/h.ics", Some("public"), None);
    let state = |refresh: i64| Fetched { url: plain.url.clone(), tried: 10, checked: 10, refresh, ..Fetched::default() };
    assert_eq!(every(&plain, &state(0)), EVERY);
    assert_eq!(every(&holidays, &Fetched { url: holidays.url.clone(), ..state(0) }), HOLIDAYS_EVERY);
    assert_eq!(every(&plain, &state(60)), SOONEST, "never sooner than an hour");
    assert_eq!(every(&plain, &state(7 * 86_400)), LATEST, "never later than a day");
    assert_eq!(every(&plain, &state(6 * 3600)), 6 * 3600);
}

/// The official files' shapes: school holidays that begin after class start
/// the day after their event's first day and end before the morning classes
/// resume; a zero-length event gives no day and no crash; the teachers' one
/// day gives none; without the box, each event's own days. Public holidays
/// count as days off by default, school holidays do not.
#[test]
fn the_official_files_shapes_give_their_days() {
    crate::config::stand_ins_only();
    let zone = jiff::tz::TimeZone::get("Europe/Paris").unwrap();
    let config: Config = toml::from_str(
        "[agenda.subscriptions.zone-test]\nurl = \"https://fr.ftp.opendatasoft.com/openscol/fr-en-calendrier-scolaire/Zone-A.ics\"\nholidays = \"school\"\nafter_class = true\n\n\
         [agenda.subscriptions.zone-plain]\nurl = \"https://calendar.example.org/zone.ics\"\nholidays = \"school\"\n\n\
         [agenda.subscriptions.public-test]\nurl = \"https://etalab.github.io/jours-feries-france-data/ics/jours_feries_metropole.ics\"\nholidays = \"public\"\n",
    )
    .unwrap();
    for (id, text) in [("zone-test", SCHOOL), ("zone-plain", SCHOOL), ("public-test", PUBLIC)] {
        vdir::prepare_subscribed(id, id, None).unwrap();
        let s = config.agenda.subscriptions.get(id).unwrap();
        assert_eq!(take(id, s, calendar(text), 1_000).unwrap(), Taken::Changed);
    }
    let all = holidays(&config, &zone);
    let of = |id: &str| all.iter().find(|h| h.id == id).unwrap();
    let after = of("zone-test");
    assert_eq!((after.holidays, after.days_off, after.as_of), (Holidays::School, false, Some(1_000)));
    assert_eq!(after.periods.iter().map(|(a, b, _)| (a.to_string(), b.to_string())).collect::<Vec<_>>(), [("2030-07-07".to_string(), "2030-09-02".to_string()), ("2030-10-20".into(), "2030-11-03".into())]);
    let plain = of("zone-plain");
    assert_eq!(plain.periods.iter().map(|(a, b, _)| (a.to_string(), b.to_string())).collect::<Vec<_>>(), [("2030-07-06".to_string(), "2030-09-02".to_string()), ("2030-09-02".into(), "2030-09-02".into()), ("2030-10-19".into(), "2030-11-03".into()), ("2031-05-09".into(), "2031-05-09".into())]);
    let public = of("public-test");
    assert!(public.days_off && public.periods.len() == 2);
    let off = crate::vacations::DaysOff::with_holidays(&[], &all);
    assert_eq!(off.on(day("2030-12-25")).map(|p| p.label.as_str()), Some("Jour de Noël"));
    assert!(off.on(day("2030-10-21")).is_none(), "school holidays count only when said");
    assert_eq!(counted_folders(&config), [folder("public-test")]);
    for id in ["zone-test", "zone-plain", "public-test"] {
        vdir::remove_subscribed(id).unwrap();
    }
}

/// An entry that does not read is passed over, never the whole
/// configuration: a subscription without an address or of another shape,
/// a vacation without its days or with no date; the rest still read.
#[test]
fn an_entry_that_does_not_read_is_passed_over() {
    let text = "[[time_off]]\nfrom = 2030-08-01\nuntil = 2030-08-15\nlabel = \"Summer\"\nkind = \"sick\"\n\n[[time_off]]\nfrom = 2030-09-01\n\n[[time_off]]\nfrom = \"tomorrow\"\nuntil = 2030-09-02\n\n\
                [agenda.subscriptions.good]\nurl = \"https://calendar.example.org/a.ics\"\n\n[agenda.subscriptions.noaddress]\nname = \"Nothing\"\n\n[agenda.subscriptions.wrong]\nurl = 3\n\n[agenda.subscriptions.\"../up\"]\nurl = \"https://calendar.example.org/b.ics\"\n";
    let config: Config = toml::from_str(text).unwrap();
    assert_eq!(config.time_off.len(), 1);
    assert_eq!((config.time_off[0].label.as_str(), config.time_off[0].kind.as_str()), ("Summer", "sick"));
    assert_eq!(config.agenda.subscriptions.keys().collect::<Vec<_>>(), ["good"]);
}

/// A subscription written, a field changed, stopped: the address checked
/// first and never written refused; what was said of its agenda goes with it.
#[test]
fn a_subscription_is_written_changed_and_stopped() {
    crate::config::stand_ins_only();
    let dir = std::env::temp_dir().join(format!("sioul-subscribed-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.toml");
    std::fs::write(&path, "# Mine\nlanguage = \"en\"\n").unwrap();
    let tr = crate::i18n::Translator::new("en");
    let config = Config::load(&path).unwrap();
    let new = New { url: "webcal://fr.ftp.opendatasoft.com/openscol/fr-en-calendrier-scolaire/Zone-B.ics".into(), name: "Zone B".into(), holidays: "school".into(), ..New::default() };
    let id = subscribe(&path, &config, &new, &tr).unwrap();
    let config = Config::load(&path).unwrap();
    let s = config.agenda.subscriptions.get(&id).unwrap();
    assert_eq!((s.url.as_str(), s.holidays(), s.after_class(), s.days_off()), ("https://fr.ftp.opendatasoft.com/openscol/fr-en-calendrier-scolaire/Zone-B.ics", Some(Holidays::School), true, false));
    assert!(std::fs::read_to_string(&path).unwrap().starts_with("# Mine"), "the file's comments kept");
    assert!(subscribe(&path, &config, &New { url: "http://calendar.example.org/x.ics".into(), ..New::default() }, &tr).is_err());
    set(&path, &id, "days_off", "true", &tr).unwrap();
    assert!(set(&path, &id, "url", "http://calendar.example.org/x.ics", &tr).is_err());
    assert!(Config::load(&path).unwrap().agenda.subscriptions.get(&id).unwrap().days_off());
    crate::agendas::set_field(&path, &place(&id), "whose", "child").unwrap();
    stop(&path, &id).unwrap();
    let config = Config::load(&path).unwrap();
    assert!(config.agenda.subscriptions.is_empty() && config.agenda.calendars.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

/// The review's F7: a calendar whose own lines are heavy does not weigh
/// that much once more per event: each event keeps what it needs (its time
/// zones, the format's lines), never the calendar's whole head.
#[test]
fn a_copy_weighs_about_what_its_answer_weighs() {
    let mut text = String::from("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Example//EN\r\n");
    text.push_str(&format!("X-FILL:{}\r\n", "a".repeat(100_000)));
    text.push_str("BEGIN:VTIMEZONE\r\nTZID:Europe/Paris\r\nBEGIN:STANDARD\r\nDTSTART:19701025T030000\r\nTZOFFSETFROM:+0200\r\nTZOFFSETTO:+0100\r\nEND:STANDARD\r\nEND:VTIMEZONE\r\n");
    text.push_str("BEGIN:VTIMEZONE\r\nTZID:Asia/Tokyo\r\nBEGIN:STANDARD\r\nDTSTART:19700101T000000\r\nTZOFFSETFROM:+0900\r\nTZOFFSETTO:+0900\r\nEND:STANDARD\r\nEND:VTIMEZONE\r\n");
    for n in 0..200 {
        text.push_str(&format!("BEGIN:VEVENT\r\nUID:{n}\r\nDTSTART;TZID=Europe/Paris:20300101T090000\r\nEND:VEVENT\r\n"));
    }
    text.push_str("END:VCALENDAR\r\n");
    let feed = read(text.as_bytes()).unwrap();
    let kept: usize = feed.items.iter().map(|(_, t)| t.len()).sum();
    assert!(kept < 4 * text.len(), "{kept} bytes kept for {} received", text.len());
    let one = &feed.items[0].1;
    assert!(one.contains("TZID:Europe/Paris") && !one.contains("Asia/Tokyo") && !one.contains("X-FILL") && one.contains("VERSION:2.0"), "{one}");
}

/// A time zone of thousands of parts named by every event would weigh a
/// thousand times what came: refused, the last copy kept, the line saying
/// the calendar is too large to keep (F7).
#[test]
fn a_calendar_too_heavy_once_kept_replaces_nothing() {
    crate::config::stand_ins_only();
    let mut heavy = String::from("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VTIMEZONE\r\nTZID:Heavy/Zone\r\n");
    for year in 0..1_500 {
        heavy.push_str(&format!("BEGIN:STANDARD\r\nDTSTART:{:04}0101T000000\r\nTZOFFSETFROM:+0100\r\nTZOFFSETTO:+0100\r\nEND:STANDARD\r\n", 1000 + year));
    }
    heavy.push_str("END:VTIMEZONE\r\n");
    for n in 0..2_000 {
        heavy.push_str(&format!("BEGIN:VEVENT\r\nUID:{n}\r\nDTSTART;TZID=Heavy/Zone:20300101T090000\r\nEND:VEVENT\r\n"));
    }
    heavy.push_str("END:VCALENDAR\r\n");
    assert_eq!(read(heavy.as_bytes()), Err(Problem::TooLargeKept));
    let id = "heavy-test";
    let s = subscription("https://calendar.example.org/heavy.ics", None, None);
    vdir::prepare_subscribed(id, "Heavy", None).unwrap();
    assert_eq!(take(id, &s, calendar(PUBLIC), 1_000).unwrap(), Taken::Changed);
    assert_eq!(take(id, &s, calendar(&heavy), 2_000).unwrap(), Taken::Kept(Problem::TooLargeKept));
    assert_eq!(folder(id).read_dir().unwrap().flatten().filter(|e| e.path().extension().is_some_and(|x| x == "ics")).count(), 2, "the last copy stays");
    let tr = crate::i18n::Translator::new("en");
    let now = jiff::Timestamp::from_second(3_000).unwrap().to_zoned(jiff::tz::TimeZone::UTC);
    assert!(line(id, &now, &tr).contains("too much room"), "{}", line(id, &now, &tr));
    vdir::remove_subscribed(id).unwrap();
}

// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Contacts and calendars from the terminal: `sioul dav add|sync`,
//! `sioul contacts`, `sioul contact new`, `sioul agenda`, `sioul event new`.

use crate::Session;
use crate::accounts::prompt_password;
use clap::Subcommand;
use jiff::Zoned;
use jiff::tz::TimeZone;
use sioul_core::agenda::{self, EventEdit};
use sioul_core::config::{self, Account};
use sioul_core::contacts::{self, ContactEdit, Labeled};
use sioul_core::{vdir, view};
use sioul_sync::dav::{self, Homes};
use sioul_sync::secret;

#[derive(Subcommand)]
pub(crate) enum DavCommand {
    /// Finds where your contacts and calendars are, asks for the password, tests it and adds the account.
    Add {
        address: String,
        /// The server's address, when it cannot be found from yours.
        #[arg(long)]
        url: Option<String>,
        /// The login, when it is not the address.
        #[arg(long)]
        username: Option<String>,
        #[arg(long)]
        id: Option<String>,
    },
    /// Syncs contacts and calendars both ways: every account, or one.
    Sync { account: Option<String> },
    /// Reads every contact and event kept: what does not parse, what a change
    /// made here would rewrite, what each collection holds. Names files, never contents.
    Check,
    /// Makes an address book; the next sync creates it on the account's server.
    NewBook { account: String, name: String },
}

pub(crate) fn run(s: &Session, command: DavCommand) -> Result<(), String> {
    match command {
        DavCommand::Add { address, url, username, id } => add(s, &address, url.as_deref(), username.as_deref(), id),
        DavCommand::Sync { account } => sync(s, account.as_deref()),
        DavCommand::Check => check(),
        DavCommand::NewBook { account, name } => {
            if s.config.account(&account).is_none_or(|a| !a.is_dav()) {
                return Err(s.say("account-unknown", &[("id", account)]));
            }
            let book = vdir::create(vdir::Kind::Contacts, &account, &name, None, &[])?;
            println!("{}/{}", book.account, book.id);
            Ok(())
        }
    }
}

fn add(s: &Session, address: &str, url: Option<&str>, username: Option<&str>, id: Option<String>) -> Result<(), String> {
    let address = address.trim().to_ascii_lowercase();
    let id = id.unwrap_or_else(|| dav_id(&s.config, &address));
    if s.config.every_account().any(|a| a.id == id) {
        return Err(s.say("account-exists", &[("id", id)]));
    }
    let login = username.unwrap_or(&address).to_string();
    // A mail account at the same domain tells where a cPanel host serves them.
    let mail_host = s.config.accounts.iter().find(|a| a.address.as_deref() == Some(address.as_str())).and_then(|a| a.host.clone());
    let password = prompt_password(s, &address)?;
    let homes = dav::test(&address, &login, &password, url, mail_host.as_deref()).map_err(|e| e.sentence(&s.tr, &id))?;
    say_homes(s, &homes);
    let found = homes.calendars.clone().or(homes.contacts.clone()).unwrap_or_default();
    let host = host_of(&found);
    let account = Account::dav(&id, &address, &host, Some(&found), Some(&login));
    secret::save(&account, &password).map_err(|e| e.sentence(&s.tr, &id))?;
    config::add_dav_account(&s.config_path, &account)?;
    homes.save(&id).map_err(|e| e.sentence(&s.tr, &id))?;
    println!("{}", s.say("dav-added", &[("id", id.clone())]));
    sync_one(s, &account)
}

/// Reads everything kept, as Sioul would, and says where it would go wrong.
fn check() -> Result<(), String> {
    use sioul_core::lines;
    for collection in vdir::collections(vdir::Kind::Contacts) {
        let (mut failed, mut changed, mut grouped, mut versions) = (Vec::new(), Vec::new(), 0, std::collections::BTreeMap::<String, usize>::new());
        let items = collection.items();
        for path in &items {
            let text = std::fs::read_to_string(path).unwrap_or_default();
            let Some(contact) = contacts::read(path, &collection) else {
                failed.push(path.file_name().unwrap_or_default().to_string_lossy().to_string());
                continue;
            };
            let version = lines::unfold(&text).iter().find(|l| lines::name(l) == "VERSION").map(|l| lines::value(l).trim().to_string()).unwrap_or_default();
            *versions.entry(version).or_default() += 1;
            grouped += usize::from(lines::unfold(&text).iter().any(|l| l.split([';', ':']).next().is_some_and(|h| h.contains('.'))));
            // A change that changes nothing: what would differ besides REV.
            let edit = ContactEdit {
                name: contact.name.clone(),
                emails: contact.emails.clone(),
                phones: contact.phones.clone(),
                org: contact.org.clone(),
                title: contact.title.clone(),
                addresses: contact.addresses.clone(),
                birthday: contact.birthday.clone(),
                notes: contact.notes.clone(),
                urls: contact.urls.clone(),
                categories: Some(contact.categories.clone()),
            };
            let before: std::collections::BTreeSet<String> = lines::unfold(&text).into_iter().filter(|l| lines::name(l) != "REV").collect();
            let after: std::collections::BTreeSet<String> = lines::unfold(&contacts::apply(&text, &edit)).into_iter().filter(|l| lines::name(l) != "REV").collect();
            let lost: Vec<String> = before.difference(&after).map(|l| lines::name(l)).collect();
            if !lost.is_empty() {
                // How lines differ, values hidden: the head (name and parameters), and whether the value reads the same.
                if std::env::var_os("SIOUL_CHECK_DETAILS").is_some() {
                    for old in before.difference(&after) {
                        let head = |l: &str| l[..l.len() - lines::value(l).len()].to_string();
                        let partner = after.iter().find(|n| lines::name(n) == lines::name(old) && lines::unescape(lines::value(n)) == lines::unescape(lines::value(old)));
                        match partner {
                            Some(new) => println!("    {} → {} (same value)", crate::one_line(&head(old)), crate::one_line(&head(new))),
                            None => println!("    {} → value differs (len {} vs {:?})", crate::one_line(&head(old)), lines::value(old).len(), after.iter().filter(|n| lines::name(n) == lines::name(old)).map(|n| format!("{}{}", head(n), lines::value(n).len())).collect::<Vec<_>>()),
                        }
                    }
                }
                changed.push((path.file_name().unwrap_or_default().to_string_lossy().to_string(), lost));
            }
        }
        println!("contacts/{}/{}: {} cards, versions {:?}, {} with grouped lines (item1.EMAIL…)", crate::one_line(&collection.account), crate::one_line(&collection.id), items.len(), versions, grouped);
        for name in &failed {
            println!("  does not parse: {}", crate::one_line(name));
        }
        let mut by_property: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
        for (_, lost) in &changed {
            for property in lost {
                *by_property.entry(property.clone()).or_default() += 1;
            }
        }
        println!("  an unchanged edit would rewrite {} cards; lines rewritten by property: {:?}", changed.len(), by_property);
    }
    let zone = TimeZone::system();
    for collection in vdir::collections(vdir::Kind::Calendars) {
        let (mut failed, mut components, mut recurring, mut rewritten) = (Vec::new(), std::collections::BTreeMap::<String, usize>::new(), 0, Vec::new());
        let items = collection.items();
        for path in &items {
            let text = std::fs::read_to_string(path).unwrap_or_default();
            let Some(ical) = agenda::parse(&text) else {
                failed.push(path.file_name().unwrap_or_default().to_string_lossy().to_string());
                continue;
            };
            for component in &ical.components {
                *components.entry(format!("{:?}", component.component_type)).or_default() += 1;
            }
            // A rule of an event or a task, not a time zone's change of hour.
            let mut inside: Vec<String> = Vec::new();
            let repeats = lines::unfold(&text).iter().any(|l| match lines::name(l).as_str() {
                "BEGIN" => {
                    inside.push(lines::value(l).trim().to_ascii_uppercase());
                    false
                }
                "END" => {
                    inside.pop();
                    false
                }
                "RRULE" => inside.last().is_some_and(|c| c == "VEVENT" || c == "VTODO"),
                _ => false,
            });
            recurring += usize::from(repeats);
            if let Some(edit) = agenda::edit_of_text(&text, &zone)
                && let Ok(again) = agenda::apply(&text, &edit, &zone)
            {
                let skip = ["DTSTAMP", "LAST-MODIFIED", "SEQUENCE"];
                let before: std::collections::BTreeSet<String> = lines::unfold(&text).into_iter().filter(|l| !skip.contains(&lines::name(l).as_str())).collect();
                let after: std::collections::BTreeSet<String> = lines::unfold(&again).into_iter().filter(|l| !skip.contains(&lines::name(l).as_str())).collect();
                let lost: Vec<String> = before.difference(&after).map(|l| lines::name(l)).collect();
                if !lost.is_empty() {
                    rewritten.push((path.file_name().unwrap_or_default().to_string_lossy().to_string(), lost));
                }
            }
        }
        println!(
            "calendars/{}/{}: {} items, holds {:?}, read-only {}, components {:?}, {} repeating",
            crate::one_line(&collection.account),
            crate::one_line(&collection.id),
            items.len(),
            collection.components,
            collection.read_only,
            components,
            recurring
        );
        for name in &failed {
            println!("  does not parse: {}", crate::one_line(name));
        }
        for (name, lost) in &rewritten {
            println!("  an unchanged edit would rewrite {}: {lost:?}", crate::one_line(name));
        }
    }
    Ok(())
}

/// "example-agenda" for jane@example.org, "example-agenda-2" when taken.
pub(crate) fn dav_id(config: &sioul_core::config::Config, address: &str) -> String {
    let label = address.rsplit('@').next().unwrap_or(address).split('.').next().unwrap_or("dav");
    let base = config::slug(&format!("{label}-agenda"));
    // Those switched off keep their id too.
    std::iter::once(base.clone()).chain((2..).map(|n| format!("{base}-{n}"))).find(|id| config.every_account().all(|a| &a.id != id)).unwrap_or(base)
}

/// "https://dav.example.org/remote.php/dav/" → "dav.example.org"; the port is kept apart.
fn host_of(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    rest.split(['/', ':']).next().unwrap_or(rest).to_string()
}

fn say_homes(s: &Session, homes: &Homes) {
    let none = s.tr.text("dav-none", None);
    println!(
        "{}",
        crate::one_line(&s.say("dav-found", &[("contacts", homes.contacts.clone().unwrap_or_else(|| none.clone())), ("calendars", homes.calendars.clone().unwrap_or(none))]))
    );
}

fn sync(s: &Session, only: Option<&str>) -> Result<(), String> {
    let accounts: Vec<&Account> = s.config.accounts.iter().filter(|a| a.is_dav() && only.is_none_or(|id| a.id == id)).collect();
    if accounts.is_empty() {
        return Err(s.tr.text("dav-nothing", None));
    }
    for account in accounts {
        if let Err(e) = sync_one(s, account) {
            eprintln!("{}", crate::plain_lines(&e));
        }
    }
    Ok(())
}

pub(crate) fn sync_one(s: &Session, account: &Account) -> Result<(), String> {
    let report = dav::sync(account).map_err(|e| e.sentence(&s.tr, &account.id))?;
    println!(
        "{}",
        s.say(
            "dav-report",
            &[("account", account.id.clone()), ("sent", report.sent.to_string()), ("received", report.received.to_string()), ("removed", report.removed.to_string())]
        )
    );
    for conflict in &report.conflicts {
        println!("{}", s.say("dav-conflict", &[("path", crate::one_line(&conflict.display().to_string()))]));
    }
    Ok(())
}

/// The contacts matching `query`, one per line: name, then address or number.
pub(crate) fn list_contacts(s: &Session, query: &str) -> Result<(), String> {
    let all = contacts::all();
    let shown = view::contacts(&all, query, "", &s.tr);
    if !shown.sentence.is_empty() {
        println!("{}", crate::one_line(&shown.sentence));
    }
    // Names and details as the cards wrote them, never a terminal's escape sequences.
    for row in shown.contacts {
        println!("{} · {} · {}", crate::one_line(&row.name), crate::one_line(&row.detail), crate::one_line(&row.key));
    }
    Ok(())
}

/// A new contact in the first address book that takes it, then sent.
pub(crate) fn new_contact(s: &Session, name: &str, emails: &[String], phones: &[String]) -> Result<(), String> {
    let book = contacts::default_book().ok_or_else(|| s.tr.text("dav-no-book", None))?;
    let edit = ContactEdit {
        name: name.to_string(),
        emails: emails.iter().map(|e| Labeled { label: String::new(), value: e.clone() }).collect(),
        phones: phones.iter().map(|p| Labeled { label: String::new(), value: p.clone() }).collect(),
        ..ContactEdit::default()
    };
    let path = contacts::new_path(&book);
    vdir::write_item(&path, &contacts::new_card(&edit))?;
    println!("{}", path.display());
    let account = s.config.account(&book.account).cloned().ok_or_else(|| s.say("account-unknown", &[("id", book.account.clone())]))?;
    sync_one(s, &account)
}

/// What comes, day by day.
pub(crate) fn list_agenda(s: &Session, days: i64) -> Result<(), String> {
    let zone = TimeZone::system();
    let today = Zoned::now().date();
    let from = today.to_zoned(zone.clone()).map_err(|e| e.to_string())?.timestamp().as_second();
    let to = from + days * 86_400 + 3_600;
    let shown = view::agenda(&agenda::occurrences(from, to), today, days, &s.tr, &zone);
    if !shown.sentence.is_empty() {
        println!("{}", crate::one_line(&shown.sentence));
        return Ok(());
    }
    for day in shown.days.iter().filter(|d| !d.events.is_empty()) {
        println!("{}", day.title);
        for event in &day.events {
            // Titles and places as invitations and shared calendars wrote them, on one line.
            let place = if event.location.is_empty() { String::new() } else { format!(" · {}", crate::one_line(&event.location)) };
            println!("  {}  {}{place}", crate::one_line(&event.when), crate::one_line(&event.summary));
        }
    }
    Ok(())
}

/// A new event in the first calendar that takes it, then sent.
pub(crate) fn new_event(s: &Session, edit: &EventEdit) -> Result<(), String> {
    let calendar = agenda::default_calendar().ok_or_else(|| s.tr.text("dav-no-calendar", None))?;
    let text = agenda::new_event(edit, &TimeZone::system())?;
    let path = agenda::new_path(&calendar);
    vdir::write_item(&path, &text)?;
    println!("{}", path.display());
    let account = s.config.account(&calendar.account).cloned().ok_or_else(|| s.say("account-unknown", &[("id", calendar.account.clone())]))?;
    sync_one(s, &account)
}

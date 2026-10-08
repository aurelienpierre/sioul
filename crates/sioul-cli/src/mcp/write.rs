// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The tools that write, on this computer only: a task added or marked
//! done, an event added, a note written, a draft saved, two things tied.
//! Nothing is sent from here. A task or an event changed goes to its server
//! with the next sync, as what you change in the window does; a draft waits
//! in Drafts until you read and send it. Nothing is deleted or overwritten.
//!
//! What an agent writes may come from mail it read, so it is checked as
//! mail is: a title, a place, a subject on one line; an address one address;
//! a tie to Sioul's own things or a web page only; a note inside the notes.
//!
//! Nothing is written into a project closed to agents, nor outside projects
//! while those are closed (`access`); a thing kept from agents is not tied to,
//! nor answered: each refusal says so plainly.

use super::access;
use super::read::{self, Desk};
use super::tools::{Answer, Args};
use crate::{Session, one_line};
use jiff::Zoned;
use jiff::civil::{Date, DateTime};
use jiff::tz::TimeZone;
use serde_json::json;
use sioul_core::agenda::{self, EventEdit};
use sioul_core::compose::{self, Draft, DraftKind};
use sioul_core::config::Account;
use sioul_core::consent::Consent;
use sioul_core::links::{self, Kind, LocalLinks, Loaded, World};
use sioul_core::porch::Lane;
use sioul_core::tasks::{self, ContactRef, Link, Status, TaskEdit};
use sioul_core::vdir::{self, Collection};
use sioul_core::{notes, taskview};
use std::path::{Path, PathBuf};

/// The longest address taken: a link, not a document.
const LONGEST_ADDRESS: usize = 2000;

/// An address as Sioul writes it, of something it knows: a message kept
/// here, a task, a note…; or a web page (`https://…`). It is written into a
/// task, an event or a note's front matter as it is, so it is one line of
/// visible characters; and the window opens it when the person clicks it, so
/// it is never a file of this computer (`file:`) nor another program's
/// scheme (`smb:`, `ms-…:`). A thing kept from agents is refused: it is not
/// tied to, whatever ties it.
fn known(world: &World, consent: &Consent, uri: &str) -> Result<String, String> {
    let uri = uri.trim();
    let shown = || one_line(&uri.chars().take(200).collect::<String>());
    // Sioul's own addresses come out percent-encoded ("Call%20with…"); anything else as it was given.
    let canonical = world.canonical(uri);
    if canonical.chars().count() > LONGEST_ADDRESS || canonical.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(format!("“{}” is not an address: one line without spaces, as mid:…, sioul:task/… or https://…", shown()));
    }
    let unknown = || format!("Sioul knows nothing at “{}”: find, search_mail, search_notes and list_tasks give addresses; a web page is https://…", shown());
    match links::kind_of(&canonical) {
        Kind::Web => Ok(canonical),
        Kind::File | Kind::Other => Err(unknown()),
        Kind::Case if world.describe(&canonical).found && !consent.is_open(&links::id_of(&canonical)) => Err(access::closed_project(&links::id_of(&canonical))),
        _ if world.describe(&canonical).found && !access::allows(consent, world, &canonical) => Err(access::kept(&format!("“{}”", shown()))),
        _ if world.describe(&canonical).found => Ok(canonical),
        _ => Err(unknown()),
    }
}

/// The projects named among addresses (`sioul:case/…`).
fn cases_among(uris: &[String]) -> Vec<String> {
    uris.iter().filter(|u| links::kind_of(u) == Kind::Case).map(|u| links::id_of(u)).collect()
}

/// Whether something new in these projects may be written for an agent: each
/// open; in none, while things outside projects are open.
fn may_write(consent: &Consent, projects: &[String]) -> Result<(), String> {
    if let Some(closed) = projects.iter().find(|p| consent.knows(p) && !consent.is_open(p)) {
        return Err(access::closed_project(closed));
    }
    if consent.allows_projects(projects.iter().map(String::as_str)) { Ok(()) } else { Err(access::OUTSIDE_CLOSED.into()) }
}

/// Addresses checked, each once.
fn known_all(world: &World, consent: &Consent, uris: &[String]) -> Result<Vec<String>, String> {
    let mut out: Vec<String> = Vec::new();
    for uri in uris {
        let uri = known(world, consent, uri)?;
        if !out.contains(&uri) {
            out.push(uri);
        }
    }
    Ok(out)
}

/// A tie held by a new task, as `Loaded::tie` writes one in a task: its
/// people as CONTACT, its cases as REFID, a note as what describes it, the rest as LINK.
fn tie_in_task(edit: &mut TaskEdit, world: &World, uri: &str) {
    let link = |uri: String, rel: &str| Link { uri, label: String::new(), rel: rel.into() };
    match links::kind_of(uri) {
        Kind::Contact => edit.contacts.push(ContactRef { name: world.describe(uri).title, uri: uri.to_string() }),
        Kind::Case if edit.cases.contains(&links::id_of(uri)) => {}
        Kind::Case => edit.cases.push(links::id_of(uri)),
        Kind::Note => edit.links.push(link(uri.to_string(), "describedby")),
        Kind::Task | Kind::Event => edit.links.push(link(format!("uid:{}", links::id_of(uri)), "related")),
        _ => edit.links.push(link(uri.to_string(), "related")),
    }
}

/// The list a new task goes into: the one named ("account/id"), else the first made for tasks.
fn target_list(s: &Session, named: Option<&str>) -> Result<Collection, String> {
    match named {
        Some(id) => tasks::lists().into_iter().find(|c| format!("{}/{}", c.account, c.id) == id && !c.read_only).ok_or_else(|| s.say("task-no-such-list", &[("list", one_line(id))])),
        None => tasks::default_list().ok_or_else(|| s.tr.text("task-no-list-yet", None)),
    }
}

/// A new item's file, under a name no other file has: never over another one.
fn free_item(mut path: PathBuf, again: impl Fn() -> PathBuf) -> PathBuf {
    while path.exists() {
        path = again();
    }
    path
}

pub fn add_task(s: &Session, args: &Args) -> Result<Answer, String> {
    let mut edit = TaskEdit {
        title: args.needed_line("title")?,
        notes: args.body("notes")?,
        start: args.moment("start")?.unwrap_or_default(),
        due: args.moment("due")?.unwrap_or_default(),
        estimate: args.number("estimate", 0, 1, 6000)? as u32,
        priority: args.number("priority", 0, 1, 9)? as u8,
        categories: args.list("tags")?.iter().map(|tag| one_line(tag)).filter(|tag| !tag.is_empty()).collect(),
        ..TaskEdit::default()
    };
    if let Some(kind) = args.line("kind")? {
        let kinds = s.config.task_kinds(&s.tr);
        if !kinds.iter().any(|(id, _)| *id == kind) {
            return Err(format!("“kind” is one of: {}; not “{kind}”.", kinds.iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>().join(", ")));
        }
        edit.kind = kind;
    }
    let list = target_list(s, args.text("list")?.as_deref())?;
    let loaded = Loaded::read(&s.config);
    let consent = access::of(s, &loaded);
    for case in args.list("cases")? {
        if !loaded.cases.iter().any(|c| c.id == case) {
            return Err(format!("No project “{}”: list_projects gives their ids.", one_line(&case)));
        }
        if !consent.is_open(&case) {
            return Err(access::closed_project(&case));
        }
        if !edit.cases.contains(&case) {
            edit.cases.push(case);
        }
    }
    // Only the tasks an agent may see are found: a step of one kept from agents is never made.
    let open_tasks: Vec<tasks::Task> = loaded.tasks.iter().filter(|t| consent.allows(&links::task_uri(&t.uid))).cloned().collect();
    let mut projects = edit.cases.clone();
    if let Some(parent) = args.text("parent")? {
        let parent = read::find_task(s, &open_tasks, &parent)?;
        edit.parent = parent.uid.clone();
        projects.extend(consent.projects_of(&links::task_uri(&parent.uid)));
    }
    for wanted in args.list("after")? {
        edit.waits_for.push(read::find_task(s, &open_tasks, &wanted)?.uid.clone());
    }
    let world = loaded.world();
    if let Some(source) = args.text("source")? {
        edit.links.push(Link { uri: known(&world, &consent, &source)?, label: String::new(), rel: "via".into() });
    }
    let tied = known_all(&world, &consent, &args.list("links")?)?;
    projects.extend(cases_among(&tied));
    may_write(&consent, &projects)?;
    for uri in tied {
        tie_in_task(&mut edit, &world, &uri);
    }
    let uid = vdir::new_name();
    let text = tasks::new_task(&edit, &uid, &TimeZone::system(), &Zoned::now())?;
    let path = free_item(tasks::new_path(&list), || tasks::new_path(&list));
    vdir::write_item(&path, &text)?;
    let uri = links::task_uri(&uid);
    let text = format!("{}\n{}  [{uid}]  <{uri}>", s.say("ui-saved", &[("path", path.display().to_string())]), edit.title);
    Ok(Answer { text, data: json!({ "uid": uid, "uri": uri, "list": format!("{}/{}", list.account, list.id), "file": path, "sent": false }) })
}

pub fn complete_task(s: &Session, args: &Args) -> Result<Answer, String> {
    let wanted = args.needed("task")?;
    let desk = Desk::read(s);
    // Only the tasks an agent may see are found: one kept from agents is never marked done by one.
    let open = desk.open_tasks();
    let task = read::find_task(s, &open, &wanted)?;
    if task.read_only {
        return Err(s.tr.text("task-read-only", None));
    }
    let said = s.tr.text("task-done-said", None);
    // Done already, and not one that comes back: nothing to write.
    if task.status == Status::Completed && task.repeat.is_empty() {
        return Ok(Answer { text: format!("{said} {}  [{}]", one_line(&task.title), task.uid), data: json!({ "uid": task.uid, "changed": false }) });
    }
    let current = std::fs::read_to_string(&task.key).map_err(|e| format!("{}: {e}", task.key))?;
    let text = tasks::set_status(&current, Status::Completed, &TimeZone::system(), &Zoned::now())?;
    vdir::write_item(Path::new(&task.key), &text)?;
    // What finishing it changed, said once.
    let after = Desk::read(s);
    let effect = taskview::done_effect(&after.context(s), &desk.plan, &task.uid);
    let comes_back = !task.repeat.is_empty();
    let text = [said, format!("{}  [{}]", one_line(&task.title), task.uid), one_line(&effect)].into_iter().filter(|l| !l.is_empty()).collect::<Vec<_>>().join("\n");
    Ok(Answer { text, data: json!({ "uid": task.uid, "changed": true, "comes_back": comes_back, "effect": effect }) })
}

pub fn add_event(s: &Session, args: &Args) -> Result<Answer, String> {
    // A new event is in no project.
    if !s.config.mcp.outside_projects {
        return Err(access::OUTSIDE_CLOSED.into());
    }
    let title = args.needed_line("title")?;
    let all_day = args.flag("all_day")?;
    let start = args.moment("start")?.ok_or("“start” is needed.")?;
    let end = args.moment("end")?.unwrap_or_else(|| start.clone());
    // `moment` gave days as "2026-10-05" and times as "2026-10-05T09:00".
    let timed = |m: &str| m.contains('T');
    let backwards = if all_day {
        if timed(&start) || timed(&end) {
            return Err("A whole-day event starts and ends on days, as 2026-10-05.".into());
        }
        end.parse::<Date>().ok() < start.parse::<Date>().ok()
    } else {
        if !timed(&start) || !timed(&end) {
            return Err("An event at a time starts and ends at times, as 2026-10-05T09:00; a day alone is all_day.".into());
        }
        end.parse::<DateTime>().ok() < start.parse::<DateTime>().ok()
    };
    if backwards {
        return Err("“end” comes before “start”.".into());
    }
    let repeat = args.text("repeat")?.unwrap_or_default();
    if !matches!(repeat.as_str(), "" | "daily" | "weekly" | "monthly" | "yearly") {
        return Err(format!("“repeat” is daily, weekly, monthly or yearly; not “{}”.", one_line(&repeat)));
    }
    let calendar = match args.text("calendar")? {
        Some(id) => vdir::collections(vdir::Kind::Calendars)
            .into_iter()
            .find(|c| format!("{}/{}", c.account, c.id) == id && !c.read_only && c.holds("VEVENT"))
            .ok_or_else(|| format!("No calendar “{}” takes events here (account/id).", one_line(&id)))?,
        None => agenda::default_calendar().ok_or_else(|| s.tr.text("dav-no-calendar", None))?,
    };
    let edit = EventEdit { title, location: args.line("location")?.unwrap_or_default(), notes: args.body("notes")?, start, end, all_day, repeat, ..EventEdit::default() };
    let zone = TimeZone::system();
    let text = agenda::new_event(&edit, &zone)?;
    let path = free_item(agenda::new_path(&calendar), || agenda::new_path(&calendar));
    vdir::write_item(&path, &text)?;
    let uid = agenda::event_ref(&text, &path.display().to_string(), &zone).map(|e| e.uid).unwrap_or_default();
    let uri = links::event_uri(&uid);
    let when = if edit.start == edit.end { edit.start.clone() } else { format!("{} – {}", edit.start, edit.end) };
    let text = format!("{}\n{} · {when}  <{uri}>", s.tr.text("event-saved", None), edit.title);
    Ok(Answer { text, data: json!({ "uid": uid, "uri": uri, "calendar": format!("{}/{}", calendar.account, calendar.id), "file": path, "sent": false }) })
}

/// The folder new notes go into: the configuration's, else "notes" (as the window does).
fn notes_folder(s: &Session) -> String {
    s.config.notes_folder.clone().filter(|f| !f.trim().is_empty()).unwrap_or_else(|| "notes".into())
}

/// A name Windows keeps for a device, whatever comes after its first dot
/// ("CON", "nul.md", "COM1"): a file or a folder of that name is no file there.
fn reserved(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or(name).trim_end().to_ascii_lowercase();
    let numbered = |prefix: &str| stem.strip_prefix(prefix).is_some_and(|n| n.len() == 1 && n != "0" && n.chars().all(|c| c.is_ascii_digit()));
    matches!(stem.as_str(), "con" | "prn" | "aux" | "nul" | "conin$" | "conout$") || numbered("com") || numbered("lpt")
}

/// The folder an agent asked for, as a folder of the notes: relative, inside,
/// each name one that every system can hold (no drive as "C:", no device as
/// "CON", no name ending with a dot or a space), no hidden folder.
fn notes_folder_asked(asked: &str) -> Result<String, String> {
    let wrong = || format!("“folder” is a folder inside the notes, as admin/letters; not “{}”.", one_line(asked));
    if asked.starts_with(['/', '\\']) {
        return Err(wrong());
    }
    let folder = notes::normalize(asked).ok_or_else(wrong)?;
    let fits = |part: &str| !(part.starts_with('.') || part.ends_with(['.', ' ']) || part.contains(['<', '>', ':', '"', '|', '?', '*']) || part.chars().any(char::is_control) || reserved(part));
    if folder.split('/').all(fits) { Ok(folder) } else { Err(wrong()) }
}

/// Whether a folder of the notes is reached without a link: a symbolic
/// link or a junction on its way would take the note out of the notes.
fn inside_notes(root: &Path, folder: &str) -> bool {
    let mut at = root.to_path_buf();
    for part in folder.split('/') {
        at.push(part);
        match std::fs::symlink_metadata(&at) {
            Ok(meta) if meta.file_type().is_symlink() => return false,
            Ok(_) => {}
            // Not made yet: `notes::write` makes it, inside.
            Err(_) => return true,
        }
    }
    true
}

/// The title a note's file is named after: at most 120 bytes of it (a file
/// name holds 255, and a number may follow), and never a name Windows keeps.
fn file_title(title: &str) -> String {
    let mut name = String::new();
    for c in title.chars() {
        if name.len() + c.len_utf8() > 120 {
            break;
        }
        name.push(c);
    }
    if reserved(notes::file_name(&name).trim_end_matches(".md")) { format!("_{name}") } else { name }
}

pub fn add_note(s: &Session, args: &Args) -> Result<Answer, String> {
    let title = args.needed_line("title")?;
    let mut body = args.body("body")?;
    let root = s.config.case_store_path().ok_or_else(|| s.tr.text("error-no-store", None))?;
    let folder = match args.text("folder")? {
        Some(asked) => notes_folder_asked(&asked)?,
        None => notes_folder(s),
    };
    if !inside_notes(&root, &folder) {
        return Err(format!("“{}” leads out of the notes folder (a link): the note is not written there.", one_line(&folder)));
    }
    let loaded = Loaded::read(&s.config);
    let consent = access::of(s, &loaded);
    let links = known_all(&loaded.world(), &consent, &args.list("links")?)?;
    // In the projects whose files hold its folder, and those it is tied to.
    let mut projects: Vec<String> = consent.note_projects(&format!("{folder}/{}", notes::file_name(&file_title(&title)))).into_iter().collect();
    projects.extend(cases_among(&links));
    may_write(&consent, &projects)?;
    // The title is written as the heading: a body starting with it again keeps one.
    if let Some(rest) = body.trim_start().strip_prefix(&format!("# {title}")).filter(|r| r.is_empty() || r.starts_with('\n')) {
        body = rest.trim_start().to_string();
    }
    let path = notes::free_path(&root, &folder, &file_title(&title));
    notes::write(&root, &path, &notes::new_text(&title, &[("links", links.clone())], &body))?;
    let uri = notes::uri_of(&path);
    let text = format!("{}\n{title}  <{uri}>", s.say("ui-saved", &[("path", path.clone())]));
    Ok(Answer { text, data: json!({ "uri": uri, "path": path, "links": links }) })
}

/// The account a message goes out from: the one asked or the message's,
/// else the first by priority, among those Sioul sends from.
fn sender<'a>(s: &'a Session, asked: Option<&str>) -> Result<&'a Account, String> {
    if let Some(id) = asked {
        return s.config.account(id).filter(|a| a.syncs()).ok_or_else(|| s.say("account-unknown", &[("id", one_line(id))]));
    }
    s.config.accounts.iter().filter(|a| a.syncs()).min_by_key(|a| a.priority).ok_or_else(|| s.tr.text("mail-no-account", None))
}

/// A draft's text: needed, and in Markdown.
fn draft_body(args: &Args) -> Result<String, String> {
    let body = args.body("body")?;
    if body.trim().is_empty() {
        return Err("“body” is needed: the text of the draft, in Markdown.".into());
    }
    Ok(body)
}

/// A recipient as the agent wrote it, checked: one address
/// ("jane@example.org", or "Jane <jane@example.org>"), and a name that can
/// neither hold another address nor pass for one, nor break a header.
fn recipient(entry: &str) -> Result<String, String> {
    let entry = entry.trim();
    let wrong = || format!("“{}” is not one address: jane@example.org, or Jane <jane@example.org>.", one_line(entry));
    let address = compose::address_of(entry).filter(|_| !entry.chars().any(char::is_control)).ok_or_else(wrong)?;
    let (local, domain) = address.split_once('@').ok_or_else(wrong)?;
    let plain = |part: &str| !part.is_empty() && !part.chars().any(|c| c.is_whitespace() || matches!(c, '@' | ',' | ';' | '<' | '>' | '"' | '(' | ')' | '[' | ']' | '\\'));
    let name = entry.rfind('<').map_or("", |at| entry[..at].trim());
    if !plain(local) || !plain(domain) || name.contains(['<', '>', '@', '"', ',', ';']) {
        return Err(wrong());
    }
    Ok(entry.to_string())
}

/// A draft saved, said as the window says it, with what it holds. Saved
/// under a name no other draft has: never over another one.
fn draft_saved(s: &Session, draft: &Draft) -> Result<Answer, String> {
    draft.save()?;
    let uri = links::draft_uri(&draft.id);
    let text = format!(
        "{}\nTo: {}\nSubject: {}\n<{uri}>\n(Not sent: the person reads it in Sioul's window and sends it themselves.)",
        s.tr.text("compose-kept", None),
        draft.to.iter().chain(&draft.cc).map(|r| one_line(r)).collect::<Vec<_>>().join(", "),
        one_line(&draft.subject)
    );
    let data = json!({ "id": draft.id, "uri": uri, "account": draft.account, "to": draft.to, "cc": draft.cc, "subject": draft.subject, "encrypt": draft.encrypt, "sent": false });
    Ok(Answer { text, data })
}

/// A new draft's own name, one no draft has yet.
fn free_draft(draft: &mut Draft) {
    while draft.path().exists() {
        draft.id = Draft::new(&draft.account).id;
    }
}

pub fn draft_reply(s: &Session, args: &Args) -> Result<Answer, String> {
    let path = read::find_message(s, &args.needed("message")?)?;
    let body = draft_body(args)?;
    // Mail kept from agents is not answered by one: a blocked sender, hostile mail to a shielded address.
    let (judged, _) = read::judged(s, &path)?;
    if judged.lane == Lane::Hostile {
        return Err("This message is hostile mail to a shielded address: Sioul keeps it from agents. The person can answer it from the window, if they choose to.".into());
    }
    let loaded = Loaded::read(&s.config);
    let consent = access::of(s, &loaded);
    // Mail kept from agents is not answered by one.
    if !consent.allows_mail(judged.card.message_id.as_deref(), Some(&judged.card)) {
        return Err(access::kept("This message"));
    }
    let kind = if args.flag("reply_all")? { DraftKind::ReplyAll } else { DraftKind::Reply };
    let links = known_all(&loaded.world(), &consent, &args.list("links")?)?;
    let (own_account, _) = read::place_of(&s.config, &path);
    let account = s.config.account(&own_account).filter(|a| a.syncs()).map_or_else(|| sender(s, None), Ok)?;
    let mut draft = compose::answer(&path, kind, &account.id, &crate::mail::own_addresses(s)).ok_or_else(|| s.tr.text("mail-message-gone", None))?;
    free_draft(&mut draft);
    // A copy of the message answered stays with the draft: it may be archived before the answer leaves.
    draft.keep_original()?;
    draft.body = body;
    draft.add_signature(account.signature.as_deref());
    draft.links = links;
    // An answer to encrypted mail is encrypted too; the window says when it cannot be.
    let encrypted = sioul_core::maildir::read_one(&path).and_then(|c| c.headers.first("Content-Type").map(|t| t.trim().to_ascii_lowercase().starts_with("multipart/encrypted"))).unwrap_or(false);
    draft.encrypt = encrypted;
    draft_saved(s, &draft)
}

pub fn draft_message(s: &Session, args: &Args) -> Result<Answer, String> {
    let to = args.list("to")?.iter().map(|entry| recipient(entry)).collect::<Result<Vec<_>, _>>()?;
    let cc = args.list("cc")?.iter().map(|entry| recipient(entry)).collect::<Result<Vec<_>, _>>()?;
    if to.is_empty() {
        return Err("“to” is needed: who it goes to.".into());
    }
    let subject = args.needed_line("subject")?;
    let body = draft_body(args)?;
    let account = sender(s, args.text("account")?.as_deref())?;
    let loaded = Loaded::read(&s.config);
    let consent = access::of(s, &loaded);
    let links = known_all(&loaded.world(), &consent, &args.list("links")?)?;
    // A new message is in the projects it is tied to; in none, outside them.
    may_write(&consent, &cases_among(&links))?;
    let mut draft = Draft::new(&account.id);
    free_draft(&mut draft);
    draft.to = to;
    draft.cc = cc;
    draft.subject = subject;
    draft.body = body;
    draft.add_signature(account.signature.as_deref());
    draft.links = links;
    draft_saved(s, &draft)
}

/// Writes what makes a tie: a note through the notes, a task or an event as an item of its collection.
fn rewrite(s: &Session, change: &links::Rewrite) -> Result<(), String> {
    match change.kind {
        Kind::Note => {
            let root = s.config.case_store_path().ok_or_else(|| s.tr.text("error-no-store", None))?;
            let path = change.path.strip_prefix(&root).map_err(|_| format!("{}: outside the notes", change.path.display()))?;
            notes::write(&root, &path.to_string_lossy().replace('\\', "/"), &change.text).map(|_| ())
        }
        _ => vdir::write_item(&change.path, &change.text),
    }
}

pub fn link(s: &Session, args: &Args) -> Result<Answer, String> {
    let loaded = Loaded::read(&s.config);
    let world = loaded.world();
    let consent = access::of(s, &loaded);
    let (from, to) = (known(&world, &consent, &args.needed("from")?)?, known(&world, &consent, &args.needed("to")?)?);
    if from == to {
        return Err("A thing is not tied to itself.".into());
    }
    // In the task, the event or the note when one can hold it; else in Sioul's own file.
    let written = match loaded.tie(&from, &to, &Zoned::now()) {
        Some(change) => {
            rewrite(s, &change)?;
            change.path.display().to_string()
        }
        None => {
            let path = LocalLinks::default_path();
            let mut local = LocalLinks::load(&path);
            local.add(&from, &to, "link");
            local.save(&path)?;
            path.display().to_string()
        }
    };
    let tied = world.describe(&to);
    let title = if tied.kind == Kind::Mail { read::mail_shown(s, read::Shield::of(&s.config).as_ref(), Path::new(&tied.key), &tied.title, "").0 } else { tied.title };
    let text = s.say("link-made", &[("title", one_line(&title))]);
    Ok(Answer { text, data: json!({ "from": from, "to": to, "written_in": written }) })
}

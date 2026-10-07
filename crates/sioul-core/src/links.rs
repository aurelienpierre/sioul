// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Links between everything: mail, tasks, events, contacts, notes, drafts,
//! budget lines and cases.
//!
//! Each link lives in the thing that makes it, in that thing's own format,
//! wherever one can: in a task or an event (LINK, RELATED-TO, CONTACT, REFID:
//! CalDAV carries them to every device and other applications keep them), in
//! a note (a Markdown link or its front matter), in a draft (its `links`), in
//! a budget line (its `links`). What none of them can hold goes in a small
//! local file, `links.toml`: a message is never changed, a shared calendar may
//! be read-only. This module gathers both directions: what a thing points at,
//! and what points at it.
//!
//! Things are named by addresses, one scheme per kind:
//! - a message: `mid:<Message-ID>` (RFC 2392);
//! - a task, an event, a contact: `sioul:task/<UID>`, `sioul:event/<UID>`, `sioul:contact/<UID>`;
//! - a note: `sioul:note/<path in the notes folder>`;
//! - a draft: `sioul:draft/<id>`; a case: `sioul:case/<id>`; a paper: `sioul:paper/<id>`;
//! - a budget line: `sioul:budget/<budget>/<position in the file>`;
//! - anything else: its URL.

use crate::agenda::EventRef;
use crate::cases::Case;
use crate::compose::Draft;
use crate::contacts::Contact;
use crate::mailindex::{MailIndex, bare_id};
use crate::notes::{self, Vault};
use crate::tasks::Task;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// What a link points at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Task,
    Event,
    Mail,
    Draft,
    Note,
    Contact,
    Budget,
    Case,
    Paper,
    /// A site kept in Sioul's Sites: a bank, a tax office, a chat.
    Site,
    Web,
    File,
    Other,
}

/// The kind of thing an address names.
pub fn kind_of(uri: &str) -> Kind {
    if uri.starts_with("mid:") {
        return Kind::Mail;
    }
    if let Some(rest) = uri.strip_prefix("sioul:") {
        return match rest.split('/').next().unwrap_or("") {
            "task" => Kind::Task,
            "event" => Kind::Event,
            "contact" => Kind::Contact,
            "note" => Kind::Note,
            "draft" => Kind::Draft,
            "budget" => Kind::Budget,
            "case" => Kind::Case,
            "paper" => Kind::Paper,
            "site" => Kind::Site,
            _ => Kind::Other,
        };
    }
    if uri.starts_with("uid:") {
        return Kind::Task;
    }
    if uri.starts_with("https://") || uri.starts_with("http://") {
        return Kind::Web;
    }
    if uri.starts_with("file:") {
        return Kind::File;
    }
    Kind::Other
}

/// A UID or an id inside an address: "/" and "%" and spaces percent-encoded,
/// letters of every language kept as they are, so `id_of` gives it back.
fn encode(id: &str) -> String {
    let mut out = String::with_capacity(id.len());
    for c in id.chars() {
        if matches!(c, '%' | '/' | ' ' | '#' | '?' | '"' | '<' | '>') || c.is_control() {
            for byte in c.encode_utf8(&mut [0; 4]).bytes() {
                out.push_str(&format!("%{byte:02X}"));
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Whether the system may be asked to open an address read from a file
/// (someone else's task or event, a note): the web, mail, a phone, a map, a
/// file on this computer. Not a share of another computer ("file://host/…",
/// which Windows reaches with your credentials), nor a scheme that starts a
/// program ("ms-msdt:", "search-ms:"): those are shown, not opened. The
/// window should ask it too before `Qt.openUrlExternally`.
pub fn openable(uri: &str) -> bool {
    let lower = uri.trim().to_ascii_lowercase();
    if let Some(rest) = lower.strip_prefix("file:") {
        let host = rest.strip_prefix("//").map(|r| r.split(['/', '\\']).next().unwrap_or(""));
        // "file:/…", "file:///…", "file://localhost/…", and "file://C:\…" as Sioul wrote it on Windows.
        return host.is_none_or(|h| h.is_empty() || h == "localhost" || (h.len() == 2 && h.ends_with(':') && h.as_bytes()[0].is_ascii_alphabetic()));
    }
    ["https://", "http://", "mailto:", "tel:", "sms:", "geo:"].iter().any(|scheme| lower.starts_with(scheme))
}

/// What the desktop would start rather than show, by its extension: programs,
/// scripts, installers, shortcuts, disk images (the list attachments are kept from).
const PROGRAMS: &[&str] = &[
    // Windows
    "exe", "com", "scr", "pif", "bat", "cmd", "msi", "msp", "mst", "ps1", "psm1", "vbs", "vbe", "js", "jse", "wsf", "wsh", "hta", "cpl", "lnk", "url", "reg", "inf", "scf", "chm", "msc", "jar", "appref-ms", "application", "appx", "appxbundle", "msix", "msixbundle", "settingcontent-ms", "library-ms", "iso", "img", "vhd", "vhdx", "py", "pyw",
    // macOS
    "app", "command", "terminal", "tool", "pkg", "mpkg", "dmg", "workflow", "scpt", "scptd", "applescript", "fileloc", "webloc", "inetloc",
    // Linux
    "desktop", "sh", "run", "appimage", "deb", "rpm", "flatpakref", "flatpakrepo", "snap",
];

/// Whether a file of this name (or path, or address) is one the desktop would start.
pub fn is_program(name: &str) -> bool {
    let name = name.trim_end_matches(['.', ' ', '/', '\\']).to_ascii_lowercase();
    let last = name.rsplit(['/', '\\']).next().unwrap_or(&name);
    last.rsplit_once('.').is_some_and(|(_, extension)| PROGRAMS.contains(&extension))
}

/// The folder a file's address is in: "file:///home/a/" for "file:///home/a/run.sh".
fn folder_of(uri: &str) -> String {
    let file = uri.trim_end_matches(['/', '\\']);
    file.rfind(['/', '\\']).map_or_else(|| uri.to_string(), |at| file[..=at].to_string())
}

pub fn task_uri(uid: &str) -> String {
    format!("sioul:task/{}", encode(uid))
}

pub fn event_uri(uid: &str) -> String {
    format!("sioul:event/{}", encode(uid))
}

pub fn contact_uri(uid: &str) -> String {
    format!("sioul:contact/{}", encode(uid))
}

pub fn paper_uri(id: &str) -> String {
    format!("sioul:paper/{}", encode(id))
}

pub fn draft_uri(id: &str) -> String {
    format!("sioul:draft/{}", encode(id))
}

/// A budget line by its budget and its position in the file.
pub fn budget_uri(budget: &str, position: usize) -> String {
    format!("sioul:budget/{}/{position}", encode(budget))
}

/// A site of Sioul's Sites, by its id.
pub fn site_uri(id: &str) -> String {
    format!("sioul:site/{}", encode(id))
}

pub fn case_uri(id: &str) -> String {
    format!("sioul:case/{}", encode(id))
}

/// `mid:` and the Message-ID, without its angle brackets (RFC 2392).
pub fn mail_uri(message_id: &str) -> String {
    format!("mid:{}", encode(&bare_id(message_id)))
}

/// The id an address of a kind carries: "sioul:task/abc" → "abc".
pub fn id_of(uri: &str) -> String {
    let rest = uri.strip_prefix("sioul:").and_then(|r| r.split_once('/')).map_or(uri, |(_, id)| id);
    let rest = rest.strip_prefix("uid:").unwrap_or(rest);
    notes::decode(rest.split('#').next().unwrap_or(rest))
}

/// A tie from one thing to another, and how.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Edge {
    pub from: String,
    pub to: String,
    /// "note" (its notes), "source" (made from), "waits" (waits for), "step"
    /// (a step of), "contact", "case", "mention" (a note names it),
    /// "answers" (a reply), "link".
    #[serde(default)]
    pub how: String,
}

/// Links Sioul keeps itself, for what cannot hold them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalLinks {
    #[serde(default, rename = "link")]
    pub links: Vec<Edge>,
}

impl LocalLinks {
    /// `$XDG_DATA_HOME/sioul/links.toml`.
    pub fn default_path() -> PathBuf {
        crate::config::data_dir().join("links.toml")
    }

    pub fn load(path: &Path) -> LocalLinks {
        std::fs::read_to_string(path).ok().and_then(|t| toml::from_str(&t).ok()).unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let fail = |e: std::io::Error| format!("{}: {e}", path.display());
        // A file there that does not read as ties (half written, edited by hand) is
        // left alone: `load` gave it as empty, and written over, every tie kept here would go.
        match std::fs::read_to_string(path) {
            Ok(text) => {
                toml::from_str::<LocalLinks>(&text).map_err(|e| format!("{}: {e}", path.display()))?;
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(fail(e)),
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(fail)?;
        }
        let temporary = path.with_extension("toml.new");
        std::fs::write(&temporary, toml::to_string(self).map_err(|e| e.to_string())?).map_err(fail)?;
        std::fs::rename(&temporary, path).map_err(fail)
    }

    /// Adds a link once.
    pub fn add(&mut self, from: &str, to: &str, how: &str) {
        let edge = Edge { from: from.to_string(), to: to.to_string(), how: how.to_string() };
        if !self.links.contains(&edge) {
            self.links.push(edge);
        }
    }

    /// Removes the links between two things, either way.
    pub fn remove(&mut self, a: &str, b: &str) {
        self.links.retain(|e| !((e.from == a && e.to == b) || (e.from == b && e.to == a)));
    }

    /// Takes out every tie between `a` and `b`, however each end was written;
    /// returns whether there was one.
    pub fn untie(&mut self, world: &World, a: &str, b: &str) -> bool {
        let (a, b) = (world.canonical(a), world.canonical(b));
        let before = self.links.len();
        self.links.retain(|e| {
            let (from, to) = (world.canonical(&e.from), world.canonical(&e.to));
            !((from == a && to == b) || (from == b && to == a))
        });
        self.links.len() != before
    }
}

/// The messages tied to a case, by bare Message-ID, and the case's id.
pub fn mail_cases(links: &LocalLinks) -> std::collections::BTreeMap<String, String> {
    let mut out = std::collections::BTreeMap::new();
    for edge in &links.links {
        for (mail, case) in [(&edge.from, &edge.to), (&edge.to, &edge.from)] {
            if mail.starts_with("mid:") && case.starts_with("sioul:case/") {
                out.insert(bare_id(mail), id_of(case));
            }
        }
    }
    out
}

/// A budget line, as links see it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BudgetRef {
    /// `sioul:budget/<budget>/<position>`.
    pub uri: String,
    /// Its label and amount, as the budget page writes them.
    pub title: String,
    pub budget: String,
    pub date: String,
    pub links: Vec<String>,
}

/// One thing tied to the one open, as the Related panel shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Related {
    pub uri: String,
    pub kind: Kind,
    pub title: String,
    /// Who, which list, which folder: names only, no sentence.
    pub detail: String,
    /// Its date, Unix seconds, when it has one (a message, an event, a deadline).
    pub when: Option<i64>,
    /// What the window opens: a file, a draft's id, an address.
    pub key: String,
    /// How it is tied, seen from the thing open: "note", "notes-of", "source",
    /// "made", "waits-for", "unblocks", "part-of", "step", "contact",
    /// "involves", "case", "in-case", "mentions", "mentioned-by", "answers",
    /// "answered-by", "link".
    pub how: String,
    /// Still there.
    pub found: bool,
    /// A program, a script or an installer on this computer: `key` is its folder, never it.
    pub program: bool,
}

/// How a tie reads from its other end.
fn inverse(how: &str) -> &'static str {
    match how {
        "note" => "notes-of",
        "source" => "made",
        "waits" => "unblocks",
        "step" => "step",
        "contact" => "involves",
        "case" => "in-case",
        "mention" => "mentioned-by",
        "answers" => "answered-by",
        _ => "link",
    }
}

/// How a tie reads from where it starts.
fn forward(how: &str) -> &'static str {
    match how {
        "note" => "note",
        "source" => "source",
        "waits" => "waits-for",
        "step" => "part-of",
        "contact" => "contact",
        "case" => "case",
        "mention" => "mentions",
        "answers" => "answers",
        _ => "link",
    }
}

/// Which tie says most, when two things are tied twice: smaller first.
fn closeness(how: &str) -> usize {
    const ORDER: &[&str] = &["waits-for", "unblocks", "part-of", "step", "source", "made", "answers", "answered-by", "note", "notes-of", "contact", "involves", "case", "in-case", "mentions", "mentioned-by", "link"];
    ORDER.iter().position(|h| *h == how).unwrap_or(ORDER.len())
}

/// Everything Sioul knows, to follow links both ways.
pub struct World<'a> {
    pub tasks: &'a [Task],
    pub events: &'a [EventRef],
    pub contacts: &'a [Contact],
    pub vault: Option<&'a Vault>,
    pub drafts: &'a [Draft],
    pub mail: &'a MailIndex,
    pub cases: &'a [Case],
    pub budget: &'a [BudgetRef],
    pub local: &'a LocalLinks,
    pub sites: &'a [crate::sites::Site],
}

impl World<'_> {
    /// One address for one thing, however it was written: `uid:` made a task's
    /// or an event's, a message's id bare, a note's path decoded and without its heading.
    pub fn canonical(&self, uri: &str) -> String {
        let uri = uri.trim();
        if let Some(uid) = uri.strip_prefix("uid:") {
            return if self.events.iter().any(|e| e.uid == uid) && !self.tasks.iter().any(|t| t.uid == uid) { event_uri(uid) } else { task_uri(uid) };
        }
        if uri.starts_with("mid:") {
            return mail_uri(uri);
        }
        if let Some(path) = notes::path_of(uri) {
            return notes::uri_of(&path);
        }
        if let Some(rest) = uri.strip_prefix("sioul:")
            && let Some((kind, id)) = rest.split_once('/')
        {
            let id = notes::decode(id.split('#').next().unwrap_or(id));
            return format!("sioul:{kind}/{}", encode(&id));
        }
        uri.to_string()
    }

    /// Every tie, from every place that holds them.
    pub fn edges(&self) -> Vec<Edge> {
        let mut out: Vec<Edge> = Vec::new();
        let mut push = |from: String, to: String, how: &str| {
            if from != to {
                out.push(Edge { from, to, how: how.to_string() });
            }
        };
        for task in self.tasks {
            let me = task_uri(&task.uid);
            for link in &task.links {
                let how = match link.rel.as_str() {
                    "describedby" => "note",
                    "via" => "source",
                    _ => "link",
                };
                push(me.clone(), self.canonical(&link.uri), how);
            }
            for relation in &task.relations {
                let other = task_uri(&relation.uid);
                match relation.kind.as_str() {
                    "DEPENDS-ON" => push(me.clone(), other, "waits"),
                    "FINISHTOSTART" | "NEXT" => push(other, me.clone(), "waits"),
                    "PARENT" => push(me.clone(), other, "step"),
                    "CHILD" => push(other, me.clone(), "step"),
                    _ => push(me.clone(), self.canonical(&format!("uid:{}", relation.uid)), "link"),
                }
            }
            for contact in task.contacts.iter().filter(|c| !c.uri.is_empty()) {
                push(me.clone(), self.canonical(&contact.uri), "contact");
            }
            for case in &task.cases {
                push(me.clone(), case_uri(case), "case");
            }
        }
        for event in self.events {
            let me = event_uri(&event.uid);
            for link in &event.links {
                let how = if link.rel == "describedby" { "note" } else if link.rel == "via" { "source" } else { "link" };
                push(me.clone(), self.canonical(&link.uri), how);
            }
            for relation in &event.related {
                push(me.clone(), self.canonical(&format!("uid:{}", relation.uid)), "link");
            }
            for case in &event.cases {
                push(me.clone(), case_uri(case), "case");
            }
            for person in &event.people {
                if let Some(contact) = crate::contacts::by_address(self.contacts, person) {
                    push(me.clone(), contact_uri(&contact.uid), "contact");
                }
            }
        }
        if let Some(vault) = self.vault {
            for note in &vault.notes {
                let me = note.uri();
                for link in &note.links {
                    let target = self.canonical(&link.target);
                    if matches!(kind_of(&target), Kind::Web | Kind::File | Kind::Other) {
                        continue;
                    }
                    let how = if kind_of(&target) == Kind::Case { "case" } else { "mention" };
                    push(me.clone(), target, how);
                }
            }
        }
        for draft in self.drafts {
            let me = draft_uri(&draft.id);
            for link in &draft.links {
                push(me.clone(), self.canonical(link), "link");
            }
            if let Some(answered) = draft.in_reply_to.as_deref().filter(|id| !id.is_empty()) {
                push(me.clone(), mail_uri(answered), "answers");
            }
        }
        for line in self.budget {
            for link in &line.links {
                push(line.uri.clone(), self.canonical(link), "source");
            }
        }
        for edge in &self.local.links {
            push(self.canonical(&edge.from), self.canonical(&edge.to), &edge.how);
        }
        let mut seen = BTreeSet::new();
        out.retain(|e| seen.insert((e.from.clone(), e.to.clone(), e.how.clone())));
        out
    }

    /// What is tied to `uri`, both ways, each once: tasks first, then events,
    /// mail, drafts, notes, contacts, budget lines, cases.
    pub fn related(&self, uri: &str) -> Vec<Related> {
        let me = self.canonical(uri);
        let mut out: Vec<Related> = Vec::new();
        for edge in self.edges() {
            let (other, how) = if edge.from == me {
                (edge.to, forward(&edge.how))
            } else if edge.to == me {
                (edge.from, inverse(&edge.how))
            } else {
                continue;
            };
            // One row per thing: the closest tie says how.
            match out.iter_mut().find(|r| r.uri == other) {
                Some(row) if closeness(how) < closeness(&row.how) => row.how = how.to_string(),
                Some(_) => {}
                None => {
                    let mut found = self.describe(&other);
                    found.how = how.to_string();
                    out.push(found);
                }
            }
        }
        out.sort_by(|a, b| (a.kind, a.when.map(|w| -w), a.title.to_lowercase()).cmp(&(b.kind, b.when.map(|w| -w), b.title.to_lowercase())));
        out
    }

    /// Things to link to: those whose title or detail holds every word typed
    /// (case and accents aside), of one kind or all, `except` left out. Titles
    /// starting with what was typed come first, then the newest. Typed nothing:
    /// the newest few of each kind.
    pub fn search(&self, query: &str, kind: Option<Kind>, except: &str, limit: usize) -> Vec<Related> {
        let folded = |text: &str| crate::text::fold(text).into_iter().collect::<String>();
        let typed = folded(query.trim());
        let words: Vec<&str> = typed.split_whitespace().collect();
        let matches = |title: &str, detail: &str| {
            let text = folded(&format!("{title} {detail}"));
            words.iter().all(|w| text.contains(w))
        };
        let want = |k: Kind| kind.is_none_or(|wanted| wanted == k);
        // (kind, starts with what was typed, date, address) for each match.
        let mut found: Vec<(Kind, bool, i64, String)> = Vec::new();
        let mut push = |k: Kind, title: &str, detail: &str, when: i64, uri: String| {
            if want(k) && matches(title, detail) {
                found.push((k, !typed.is_empty() && folded(title).starts_with(&typed), when, uri));
            }
        };
        for task in self.tasks {
            push(Kind::Task, &task.title, &task.list, 0, task_uri(&task.uid));
        }
        for event in self.events {
            push(Kind::Event, &event.summary, "", event.start, event_uri(&event.uid));
        }
        for (id, mail) in self.mail.iter() {
            push(Kind::Mail, &mail.subject, &mail.from, mail.date, mail_uri(id));
        }
        for note in self.vault.map_or(&[][..], |v| &v.notes[..]) {
            push(Kind::Note, &note.title, &note.path, note.modified, note.uri());
        }
        for contact in self.contacts {
            let emails: Vec<&str> = contact.emails.iter().map(|e| e.value.as_str()).collect();
            push(Kind::Contact, &contact.name, &format!("{} {}", contact.org, emails.join(" ")), 0, contact_uri(&contact.uid));
        }
        for line in self.budget {
            push(Kind::Budget, &line.title, &line.budget, 0, line.uri.clone());
        }
        for site in self.sites {
            push(Kind::Site, &site.name, &crate::sites::host_of(&site.url), 0, site_uri(&site.id));
        }
        for case in self.cases {
            push(Kind::Case, &case.title, &case.id, 0, case_uri(&case.id));
        }
        let except = self.canonical(except);
        found.retain(|(_, _, _, uri)| *uri != except);
        found.sort_by(|a, b| (!a.1, std::cmp::Reverse(a.2), a.0).cmp(&(!b.1, std::cmp::Reverse(b.2), b.0)));
        // Typed nothing: a few of each kind rather than a flood of one.
        if words.is_empty() {
            let mut per_kind: std::collections::BTreeMap<Kind, usize> = Default::default();
            found.retain(|(k, ..)| {
                let n = per_kind.entry(*k).or_default();
                *n += 1;
                *n <= 6
            });
        }
        found.into_iter().take(limit).map(|(_, _, _, uri)| self.describe(&uri)).collect()
    }

    /// A thing by its address: its title, a detail, a date, and what opens it.
    pub fn describe(&self, uri: &str) -> Related {
        let uri = self.canonical(uri);
        let kind = kind_of(&uri);
        let id = id_of(&uri);
        let mut out = Related { uri: uri.clone(), kind, title: id.clone(), detail: String::new(), when: None, key: String::new(), how: String::new(), found: false, program: false };
        match kind {
            Kind::Task => {
                if let Some(task) = self.tasks.iter().find(|t| t.uid == id) {
                    out.title = task.title.clone();
                    out.detail = task.list.clone();
                    out.key = task.key.clone();
                    out.found = true;
                }
            }
            Kind::Event => {
                if let Some(event) = self.events.iter().find(|e| e.uid == id) {
                    out.title = event.summary.clone();
                    out.when = (event.start > 0).then_some(event.start);
                    out.key = event.key.clone();
                    out.found = true;
                }
            }
            Kind::Mail => {
                if let Some(mail) = self.mail.get(&uri) {
                    out.title = if mail.subject.trim().is_empty() { id.clone() } else { mail.subject.clone() };
                    out.detail = mail.from.clone();
                    out.when = (mail.date > 0).then_some(mail.date);
                    out.key = mail.path.display().to_string();
                    out.found = true;
                }
            }
            Kind::Draft => {
                if let Some(draft) = self.drafts.iter().find(|d| d.id == id) {
                    out.title = draft.subject.clone();
                    out.detail = draft.to.join(", ");
                    out.key = draft.id.clone();
                    out.found = true;
                }
            }
            Kind::Note => {
                if let (Some(vault), Some(path)) = (self.vault, notes::path_of(&uri)) {
                    if let Some(note) = vault.note(&path) {
                        out.title = note.title.clone();
                        out.detail = note.folder().to_string();
                        out.key = vault.root.join(&note.path).display().to_string();
                        out.found = true;
                    }
                }
            }
            Kind::Contact => {
                if let Some(contact) = self.contacts.iter().find(|c| c.uid == id) {
                    out.title = contact.name.clone();
                    out.detail = if contact.org.is_empty() { contact.emails.first().map(|e| e.value.clone()).unwrap_or_default() } else { contact.org.clone() };
                    out.key = contact.key.clone();
                    out.found = true;
                }
            }
            Kind::Budget => {
                if let Some(line) = self.budget.iter().find(|l| l.uri == uri) {
                    out.title = line.title.clone();
                    out.detail = line.budget.clone();
                    out.key = line.uri.clone();
                    out.found = true;
                }
            }
            Kind::Case => {
                if let Some(case) = self.cases.iter().find(|c| c.id == id) {
                    out.title = case.title.clone();
                    out.detail = case.status.clone().unwrap_or_default();
                    out.key = case.id.clone();
                    out.found = true;
                }
            }
            Kind::Site => {
                if let Some(site) = self.sites.iter().find(|s| s.id == id) {
                    out.title = site.name.clone();
                    out.detail = crate::sites::host_of(&site.url);
                    out.key = site.id.clone();
                    out.found = true;
                }
            }
            // A paper of the wallet: named by its link's label where the item gives one.
            Kind::Paper => {
                out.title = id.clone();
                out.key = id.clone();
                out.found = true;
            }
            Kind::Web | Kind::File | Kind::Other => {
                let file = uri.get(..5).is_some_and(|s| s.eq_ignore_ascii_case("file:"));
                out.title = if file {
                    // A file by its name, not by its host (which a file has none of).
                    notes::decode(uri.trim_end_matches(['/', '\\']).rsplit(['/', '\\']).next().unwrap_or(&uri))
                } else {
                    uri.split("://").nth(1).map_or(uri.as_str(), |rest| rest.split('/').next().unwrap_or(rest)).to_string()
                };
                out.detail = uri.clone();
                // A program, a script, an installer is never started from a link (a synced
                // task's is someone else's word): its folder opens instead, to start it from there.
                out.program = file && is_program(&notes::decode(&uri));
                // What opens it (`openThing` hands the key to the system): only what is safe to.
                out.key = match (openable(&uri), out.program) {
                    (false, _) => String::new(),
                    (true, true) => folder_of(&uri),
                    (true, false) => uri.clone(),
                };
                out.found = true;
            }
        }
        out
    }
}

/// Everything links reach, read from disk.
#[derive(Default)]
pub struct Loaded {
    pub tasks: Vec<Task>,
    pub events: Vec<EventRef>,
    pub contacts: Vec<Contact>,
    pub vault: Option<Vault>,
    pub drafts: Vec<Draft>,
    pub mail: MailIndex,
    pub cases: Vec<Case>,
    pub budget: Vec<BudgetRef>,
    pub local: LocalLinks,
    pub sites: Vec<crate::sites::Site>,
}

/// A file to write so that a tie is made or undone: the thing that holds it, and its new text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rewrite {
    pub kind: Kind,
    pub path: PathBuf,
    pub text: String,
}

impl Loaded {
    /// The file of an event whose calendar can be written.
    fn writable_event(&self, uid: &str) -> Option<PathBuf> {
        let event = self.events.iter().find(|e| e.uid == uid)?;
        let path = PathBuf::from(&event.key);
        crate::vdir::collections(crate::vdir::Kind::Calendars).into_iter().find(|c| path.starts_with(&c.dir)).filter(|c| !c.read_only).map(|_| path)
    }

    /// The line a task or an event holds to point at `other`.
    fn tie_line(&self, other: &str, in_task: bool) -> String {
        use crate::tasks::{ContactRef, Link, contact_line, link_line};
        let id = id_of(other);
        match kind_of(other) {
            Kind::Contact if in_task => contact_line(&ContactRef { name: self.world().describe(other).title, uri: other.to_string() }),
            Kind::Case => format!("REFID:{}", crate::lines::escape(&id)),
            Kind::Note => link_line(&Link { uri: other.to_string(), label: String::new(), rel: "describedby".into() }),
            Kind::Task | Kind::Event => link_line(&Link { uri: format!("uid:{id}"), label: String::new(), rel: "related".into() }),
            _ => link_line(&Link { uri: other.to_string(), label: String::new(), rel: "related".into() }),
        }
    }

    /// `holder` with a tie to `other`, when it can hold one: a task of a list
    /// that can be written, an event of such a calendar, a note.
    fn holding(&self, holder: &str, other: &str, now: &jiff::Zoned) -> Option<Rewrite> {
        let id = id_of(holder);
        match kind_of(holder) {
            Kind::Task => {
                let task = self.tasks.iter().find(|t| t.uid == id).filter(|t| !t.read_only)?;
                let text = std::fs::read_to_string(&task.key).ok()?;
                let text = crate::tasks::add_lines(&text, &[self.tie_line(other, true)], now).ok()?;
                Some(Rewrite { kind: Kind::Task, path: PathBuf::from(&task.key), text })
            }
            Kind::Event => {
                let path = self.writable_event(&id)?;
                let text = std::fs::read_to_string(&path).ok()?;
                let text = crate::agenda::add_lines(&text, &[self.tie_line(other, false)])?;
                Some(Rewrite { kind: Kind::Event, path, text })
            }
            Kind::Note => {
                let vault = self.vault.as_ref()?;
                let path = vault.file(&notes::path_of(holder)?)?;
                let text = std::fs::read_to_string(&path).ok()?;
                Some(Rewrite { kind: Kind::Note, path, text: notes::add_front_link(&text, other) })
            }
            _ => None,
        }
    }

    /// Where a tie between `a` and `b` is written: in `a` when it can hold
    /// it, else in `b`; None when neither can (a message, a read-only
    /// calendar): it then goes in the local file.
    pub fn tie(&self, a: &str, b: &str, now: &jiff::Zoned) -> Option<Rewrite> {
        let world = self.world();
        let (a, b) = (world.canonical(a), world.canonical(b));
        if a == b {
            return None;
        }
        self.holding(&a, &b, now).or_else(|| self.holding(&b, &a, now))
    }

    /// What undoes a tie between `a` and `b`, wherever it is written: a
    /// task's LINK, CONTACT or REFID, an event's LINK, a note's front matter.
    /// The local file is the caller's (`LocalLinks::untie`). A step, a wait,
    /// a guest or a link in a note's text stay: they are changed where they are.
    pub fn untie(&self, a: &str, b: &str, now: &jiff::Zoned) -> Vec<Rewrite> {
        let world = self.world();
        let (a, b) = (world.canonical(a), world.canonical(b));
        let points_at = |line: &str, other: &str| -> bool {
            let value = crate::lines::value(line).trim();
            match crate::lines::name(line).as_str() {
                "LINK" if crate::lines::param(line, "VALUE").is_some_and(|v| v.eq_ignore_ascii_case("UID")) => world.canonical(&format!("uid:{value}")) == other,
                "LINK" => world.canonical(value) == other,
                "CONTACT" => crate::lines::param(line, "ALTREP").is_some_and(|uri| world.canonical(&uri) == other),
                "REFID" => case_uri(&crate::lines::unescape(value)) == other,
                _ => false,
            }
        };
        let mut out = Vec::new();
        for (holder, other) in [(&a, &b), (&b, &a)] {
            let id = id_of(holder);
            match kind_of(holder) {
                Kind::Task => {
                    if let Some(task) = self.tasks.iter().find(|t| t.uid == id).filter(|t| !t.read_only)
                        && let Ok(text) = std::fs::read_to_string(&task.key)
                        && crate::lines::unfold(&text).iter().any(|l| points_at(l, other))
                        && let Ok(less) = crate::tasks::remove_lines(&text, |l| points_at(l, other), now)
                    {
                        out.push(Rewrite { kind: Kind::Task, path: PathBuf::from(&task.key), text: less });
                    }
                }
                Kind::Event => {
                    if let Some(path) = self.writable_event(&id)
                        && let Ok(text) = std::fs::read_to_string(&path)
                        && let Some(less) = crate::agenda::remove_lines(&text, |l| points_at(l, other))
                        && less != text
                    {
                        out.push(Rewrite { kind: Kind::Event, path, text: less });
                    }
                }
                Kind::Note => {
                    if let Some(vault) = self.vault.as_ref()
                        && let Some(path) = notes::path_of(holder).and_then(|p| vault.file(&p))
                        && let Ok(text) = std::fs::read_to_string(&path)
                    {
                        let (less, removed) = notes::remove_front_link(&text, |v| world.canonical(v) == **other);
                        if removed {
                            out.push(Rewrite { kind: Kind::Note, path, text: less });
                        }
                    }
                }
                _ => {}
            }
        }
        out
    }

    /// Reads every task, event, contact, draft and link, the notes of the case
    /// store, and the headers of the mail kept for the configured accounts.
    pub fn read(config: &crate::config::Config) -> Loaded {
        let zone = jiff::tz::TimeZone::system();
        let root = config.case_store_path();
        let mail_roots: Vec<PathBuf> = config.accounts.iter().filter(|a| a.kind == crate::config::AccountKind::Imap).map(|a| a.maildir_path()).collect();
        let budget = root
            .as_deref()
            .and_then(|r| crate::budget::Ledger::load_with_bank(r).ok())
            .map(|ledger| {
                let budgets: Vec<(String, String)> = ledger.budgets.iter().map(|b| (b.id.clone(), b.title.clone())).collect();
                ledger
                    .lines
                    .iter()
                    .enumerate()
                    .map(|(i, line)| BudgetRef {
                        uri: budget_uri(&line.budget, i),
                        title: line.label.clone(),
                        budget: budgets.iter().find(|(id, _)| *id == line.budget).map_or_else(|| line.budget.clone(), |(_, t)| t.clone()),
                        date: line.date.to_string(),
                        links: line.links.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default();
        Loaded {
            tasks: crate::tasks::all(&zone),
            events: crate::agenda::event_refs(),
            contacts: crate::contacts::all(),
            cases: root.as_deref().and_then(|r| crate::cases::CaseStore::load(r).ok()).map(|s| s.cases).unwrap_or_default(),
            vault: root.as_deref().map(Vault::open),
            drafts: Draft::all(),
            mail: MailIndex::build(&mail_roots, &crate::config::cache_dir().join("mail-index.tsv")),
            budget,
            local: LocalLinks::load(&LocalLinks::default_path()),
            sites: crate::sites::sites(config),
        }
    }

    pub fn world(&self) -> World<'_> {
        World {
            tasks: &self.tasks,
            events: &self.events,
            contacts: &self.contacts,
            vault: self.vault.as_ref(),
            drafts: &self.drafts,
            mail: &self.mail,
            cases: &self.cases,
            budget: &self.budget,
            local: &self.local,
            sites: &self.sites,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tasks::{ContactRef, Link, Relation};

    #[test]
    fn ties_go_where_they_can_be_written() {
        let dir = std::env::temp_dir().join(format!("sioul-ties-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("vault")).unwrap();
        let task_file = dir.join("t.ics");
        std::fs::write(&task_file, "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VTODO\r\nUID:t\r\nSUMMARY:Call the CAF\r\nEND:VTODO\r\nEND:VCALENDAR\r\n").unwrap();
        std::fs::write(dir.join("vault/plan.md"), "# Plan\n").unwrap();
        let task = crate::tasks::task_of_text(&std::fs::read_to_string(&task_file).unwrap(), &jiff::tz::TimeZone::UTC).map(|mut t| {
            t.key = task_file.display().to_string();
            t
        });
        let loaded = Loaded {
            tasks: task.into_iter().collect(),
            vault: Some(Vault::open(&dir.join("vault"))),
            contacts: vec![Contact { uid: "jane".into(), name: "Jane".into(), ..Contact::default() }],
            ..Loaded::default()
        };
        let now = jiff::Zoned::now();
        // Into the task: it syncs to the phone.
        let tied = loaded.tie("sioul:task/t", "mid:abc@example.org", &now).unwrap();
        assert_eq!((tied.kind, tied.path.clone()), (Kind::Task, task_file.clone()));
        assert!(tied.text.contains("LINK;LINKREL=related;VALUE=URI:mid:abc@example.org"), "{}", tied.text);
        let to_jane = loaded.tie("mid:abc@example.org", "sioul:contact/jane", &now);
        assert_eq!(to_jane, None, "a message and a contact: the local file");
        let jane = loaded.tie("sioul:contact/jane", "sioul:task/t", &now).unwrap();
        assert!(jane.text.contains("CONTACT;ALTREP=\"sioul:contact/jane\":Jane"), "{}", jane.text);
        // Into the note when the task is not one of the two.
        let noted = loaded.tie("mid:abc@example.org", "sioul:note/plan.md", &now).unwrap();
        assert_eq!(noted.kind, Kind::Note);
        assert!(noted.text.starts_with("---\nlinks:\n  - mid:abc@example.org\n---"), "{}", noted.text);
        // Undone where written.
        std::fs::write(&task_file, &tied.text).unwrap();
        std::fs::write(&noted.path, &noted.text).unwrap();
        let loaded = Loaded { vault: Some(Vault::open(&dir.join("vault"))), ..loaded };
        let undone = loaded.untie("mid:abc@example.org", "sioul:task/t", &now);
        assert_eq!(undone.len(), 1);
        assert!(!undone[0].text.contains("mid:abc@example.org"));
        let undone = loaded.untie("sioul:note/plan.md", "mid:abc@example.org", &now);
        assert_eq!(undone[0].text, "# Plan\n");
        // Ties kept here: a file that does not read is never written over with the one tie added.
        let kept = dir.join("links.toml");
        std::fs::write(&kept, "[[link]]\nfrom = \"mid:a@example.org\"\nto = ").unwrap();
        let mut local = LocalLinks::load(&kept);
        local.add("mid:a@example.org", "sioul:case/x", "case");
        assert!(local.save(&kept).is_err());
        assert_eq!(std::fs::read_to_string(&kept).unwrap(), "[[link]]\nfrom = \"mid:a@example.org\"\nto = ");
        std::fs::remove_file(&kept).unwrap();
        local.save(&kept).unwrap();
        assert_eq!(LocalLinks::load(&kept), local);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ties_read_both_ways() {
        let certificate = Task {
            uid: "certificate".into(),
            title: "Ask for the certificate".into(),
            key: "/cal/certificate.ics".into(),
            links: vec![
                Link { uri: "sioul:note/admin/letters.md".into(), label: String::new(), rel: "describedby".into() },
                Link { uri: "mid:<abc@caf.example>".into(), label: String::new(), rel: "via".into() },
                Link { uri: "uid:meeting".into(), label: String::new(), rel: "related".into() },
            ],
            relations: vec![Relation { kind: "DEPENDS-ON".into(), uid: "form".into(), gap: 0 }],
            contacts: vec![ContactRef { name: "School".into(), uri: "sioul:contact/school".into() }],
            cases: vec!["school".into()],
            ..Task::default()
        };
        let form = Task { uid: "form".into(), title: "Fill the form".into(), ..Task::default() };
        let meeting = EventRef { uid: "meeting".into(), summary: "Meeting".into(), start: 1_791_000_000, ..EventRef::default() };
        let vault = Vault {
            root: PathBuf::from("/vault"),
            notes: vec![notes::read("admin/letters.md", "# Outbox\n\nSee [the task](sioul:task/certificate).\n"), notes::read("plan.md", "# Plan\n\n[[letters]]\n")],
            folders: vec!["admin".into()],
        };
        let mut local = LocalLinks::default();
        local.add("mid:abc@caf.example", "sioul:contact/caf", "contact");
        let (tasks, events) = (vec![certificate, form], vec![meeting]);
        let world = World { tasks: &tasks, events: &events, contacts: &[], vault: Some(&vault), drafts: &[], mail: &MailIndex::default(), cases: &[], budget: &[], local: &local, sites: &[] };
        let related = world.related("sioul:task/certificate");
        let seen: Vec<(Kind, &str, &str)> = related.iter().map(|r| (r.kind, r.how.as_str(), r.uri.as_str())).collect();
        assert_eq!(
            seen,
            vec![
                (Kind::Task, "waits-for", "sioul:task/form"),
                (Kind::Event, "link", "sioul:event/meeting"),
                (Kind::Mail, "source", "mid:abc@caf.example"),
                (Kind::Note, "note", "sioul:note/admin/letters.md"),
                (Kind::Contact, "contact", "sioul:contact/school"),
                (Kind::Case, "case", "sioul:case/school"),
            ]
        );
        assert_eq!(world.related("sioul:task/form")[0].how, "unblocks");
        // The mail knows the task it made, and the contact kept locally.
        let mail: Vec<String> = world.related("mid:<abc@caf.example>").into_iter().map(|r| r.how).collect();
        assert_eq!(mail, vec!["made", "contact"]);
        assert_eq!(world.describe("sioul:task/form").title, "Fill the form");
        assert!(!world.describe("sioul:task/gone").found);
        assert_eq!(id_of(&task_uri("a/b c")), "a/b c");
        // Searching: words anywhere, accents aside, the thing itself left out.
        let found: Vec<String> = world.search("meeting", None, "", 10).into_iter().map(|r| r.uri).collect();
        assert_eq!(found, vec!["sioul:event/meeting"]);
        let found: Vec<String> = world.search("CERTIFICATE ask", Some(Kind::Task), "", 10).into_iter().map(|r| r.uri).collect();
        assert_eq!(found, vec!["sioul:task/certificate"]);
        assert!(world.search("form", None, "sioul:task/form", 10).is_empty());
        assert_eq!(world.search("", None, "", 10).len(), 5, "typed nothing: some of each kind");
        // Accented ids and notes keep their address: read back as they were, the same once made canonical.
        assert_eq!(id_of(&task_uri("tâche/1")), "tâche/1");
        assert_eq!(world.canonical(&notes::uri_of("Santé/impôts.md")), notes::uri_of("Santé/impôts.md"));
        // Handed to the system only when safe: the web, a file here; not a program's scheme, nor another computer's share.
        assert_eq!(world.describe("https://example.org/a").key, "https://example.org/a");
        assert_eq!(world.describe("file:///home/a/scan.pdf").key, "file:///home/a/scan.pdf");
        assert!(world.describe("ms-msdt:/id PCWDiagnostic").key.is_empty() && world.describe("file://attacker.example/share/x").key.is_empty());
        // A program on this computer, from a synced task: shown by its name, its folder opens, never it.
        let program = world.describe("file:///home/a/Downloads/update%20now.desktop");
        assert_eq!((program.title.as_str(), program.key.as_str(), program.program), ("update now.desktop", "file:///home/a/Downloads/", true));
        assert!(is_program("C:\\Users\\a\\setup.EXE") && is_program("/Applications/Tool.app/") && !is_program("/home/a/scan.pdf"));
    }
}

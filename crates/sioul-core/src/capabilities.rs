// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! What each kind of server keeps, so that moving a task or a contact never
//! loses something silently, and so that what a server cannot do shows greyed
//! with its reason rather than hidden (docs/google.md).
//!
//! A CalDAV or CardDAV server keeps everything Sioul writes. Google keeps
//! less: its CalDAV holds events but no tasks and makes no calendar; its
//! CardDAV writes vCard 3.0 and drops labels and birthdays without a year;
//! Google Tasks keeps a title, notes, a status, a date and one level of steps.

use crate::config::{Account, AccountKind};
use crate::tasks::Task;
use crate::vdir::{Collection, Kind};

/// Who keeps a collection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    /// This computer only.
    Local,
    /// A CalDAV/CardDAV server that keeps what it is given.
    Dav,
    /// Google, over its CalDAV and CardDAV.
    Google,
    /// Google Tasks, over its own API.
    GoogleTasks,
}

/// Who keeps an account's collections.
pub fn provider_of(account: Option<&Account>) -> Provider {
    match account {
        None => Provider::Local,
        Some(a) if a.auth.as_deref() == Some("google") => Provider::Google,
        Some(a) if a.host.as_deref().is_some_and(|h| h.ends_with("googleusercontent.com") || h.ends_with("googleapis.com") || h == "www.google.com") => Provider::Google,
        Some(a) if a.kind == AccountKind::Dav => Provider::Dav,
        Some(_) => Provider::Local,
    }
}

/// Who keeps a collection: a Google account's task lists are Google Tasks'.
pub fn provider_of_collection(account: Option<&Account>, collection: &Collection) -> Provider {
    match provider_of(account) {
        Provider::Google if collection.kind == Kind::Calendars && collection.components.iter().any(|c| c.eq_ignore_ascii_case("VTODO")) => Provider::GoogleTasks,
        provider => provider,
    }
}

/// The task form's fields a list kept by `provider` does not keep: shown
/// greyed, with why. "steps" when the task is a step already (one level only).
pub fn task_fields_lost(provider: Provider, is_step: bool) -> Vec<&'static str> {
    if provider != Provider::GoogleTasks {
        return Vec::new();
    }
    let mut lost = vec!["start", "estimate", "project", "kind", "office", "categories", "billable", "energy", "repeat", "waits", "margins", "costs"];
    if is_step {
        lost.push("steps");
    }
    lost
}

/// Whether a provider can make, rename and delete a calendar or an address
/// book; Google Tasks makes task lists.
pub fn makes_collections(provider: Provider) -> bool {
    !matches!(provider, Provider::Google)
}

/// What a task uses that a list kept by `provider` would not keep, as words'
/// keys ("waits", "repeat"…): what to say before moving it there.
pub fn task_losses(task: &Task, to: Provider) -> Vec<&'static str> {
    if to != Provider::GoogleTasks {
        return Vec::new();
    }
    let mut lost = Vec::new();
    let mut add = |used: bool, what: &'static str| {
        if used {
            lost.push(what);
        }
    };
    add(task.relations.iter().any(|r| !r.kind.is_empty() && !r.kind.eq_ignore_ascii_case("PARENT") && !r.kind.eq_ignore_ascii_case("CHILD")), "waits");
    add(!task.start.is_empty(), "start");
    add(task.due.contains('T'), "due-time");
    add(!task.repeat.is_empty(), "repeat");
    add(task.priority > 0, "priority");
    add(!task.kind.is_empty() || task.office_hours, "kind");
    add(task.estimate > 0, "estimate");
    add(!task.links.is_empty(), "links");
    add(!task.contacts.is_empty(), "contacts");
    add(!task.categories.is_empty(), "categories");
    add(task.billable.is_some(), "billable");
    add(!task.energy.is_empty(), "energy");
    add(!task.margins.is_empty(), "margins");
    add(task.demands != crate::demands::Demands::default(), "costs");
    lost
}

/// What a contact's card uses that an address book kept by `provider` would
/// not keep: Google writes vCard 3.0, and drops labels, birthdays without a
/// year, and what only vCard 4.0 has.
pub fn contact_losses(card: &str, to: Provider) -> Vec<&'static str> {
    if to != Provider::Google {
        return Vec::new();
    }
    let lines: Vec<String> = crate::lines::unfold(card).into_iter().map(|l| l.to_ascii_uppercase()).collect();
    let has = |test: &dyn Fn(&str) -> bool| lines.iter().any(|l| test(l));
    let mut lost = Vec::new();
    if has(&|l| l.contains("X-ABLABEL")) {
        lost.push("labels");
    }
    if has(&|l| l.starts_with("BDAY") && l.split_once(':').is_some_and(|(_, v)| v.starts_with("--"))) {
        lost.push("birthday-no-year");
    }
    if has(&|l| ["KIND", "GENDER", "ANNIVERSARY", "MEMBER", "RELATED", "LANG"].iter().any(|p| l.starts_with(&format!("{p}:")) || l.starts_with(&format!("{p};")))) {
        lost.push("vcard4");
    }
    lost
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn google_keeps_less() {
        let card = "BEGIN:VCARD\r\nVERSION:4.0\r\nFN:Jane\r\nitem1.EMAIL:jane@example.org\r\nitem1.X-ABLabel:Studio\r\nBDAY:--0412\r\nGENDER:F\r\nEND:VCARD\r\n";
        assert_eq!(contact_losses(card, Provider::Google), vec!["labels", "birthday-no-year", "vcard4"]);
        assert!(contact_losses(card, Provider::Dav).is_empty());
        let task = Task { repeat: "weekly".into(), estimate: 30, ..Task::default() };
        assert_eq!(task_losses(&task, Provider::GoogleTasks), vec!["repeat", "estimate"]);
        assert!(task_losses(&task, Provider::Dav).is_empty());
        assert!(!makes_collections(Provider::Google) && makes_collections(Provider::Dav) && makes_collections(Provider::GoogleTasks));
        assert!(task_fields_lost(Provider::GoogleTasks, true).contains(&"steps") && task_fields_lost(Provider::Dav, true).is_empty());
    }
}

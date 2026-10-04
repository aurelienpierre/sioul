// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The vocabulary of sorting: what a rule may propose.
//!
//! Sioul's own rules and the Virtual Secretary bridge (docs/virtual-secretary.md)
//! speak the same words, so the Porch can show every proposal with its reason,
//! and apply it alone only when you allowed that rule to (trust is built rule
//! by rule, never assumed).

use crate::card::Card;
use serde::Deserialize;

/// What a rule may propose for a message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Put the message in a case, by id.
    AssignCase(String),
    /// File it on a shelf, out of the way.
    File(Shelf),
    /// Set it aside, with the reason.
    SetAside(String),
    /// A free label, like Virtual Secretary's tags or IMAP keywords.
    Label(String),
    /// Teach the classifier it is junk, and set it aside.
    Junk,
    /// To the trash, recoverable for 30 days.
    Trash,
    Flag(Flag),
    /// Propose a task in the message's case.
    ProposeTask { title: String, due: Option<String> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shelf {
    Newsletters,
    Notifications,
    Receipts,
    Calendar,
}

impl Shelf {
    /// Shelf names as written in the configuration.
    pub fn parse(name: &str) -> Option<Shelf> {
        match name.trim().to_ascii_lowercase().as_str() {
            "newsletters" => Some(Shelf::Newsletters),
            "notifications" => Some(Shelf::Notifications),
            "receipts" => Some(Shelf::Receipts),
            "calendar" => Some(Shelf::Calendar),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flag {
    Seen,
    Answered,
    Important,
}

/// A rule proposes; whether it may act alone is your choice, rule by rule.
pub trait Rule {
    fn name(&self) -> &str;
    fn propose(&self, card: &Card) -> Vec<Action>;
}

/// A proposal as the Porch shows it.
#[derive(Debug, Clone)]
pub struct Proposal {
    pub rule: String,
    pub actions: Vec<Action>,
    /// Applied without asking, because you allowed this rule to.
    pub automatic: bool,
}

/// Virtual Secretary, compatibility mode A: it keeps sorting on the server;
/// the IMAP folder it moved a message to becomes a proposal here.
#[derive(Debug, Clone, Deserialize)]
pub struct FolderMapping {
    /// The IMAP folder, as Virtual Secretary names it: "INBOX.Money.Taxes".
    pub folder: String,
    #[serde(default)]
    pub case: Option<String>,
    #[serde(default)]
    pub shelf: Option<String>,
    #[serde(default)]
    pub junk: bool,
}

impl FolderMapping {
    /// What a message found in `folder` should become.
    pub fn actions_for(mappings: &[FolderMapping], folder: &str) -> Vec<Action> {
        let Some(m) = mappings.iter().find(|m| m.folder.eq_ignore_ascii_case(folder)) else { return Vec::new() };
        let case = m.case.clone().map(Action::AssignCase);
        let shelf = m.shelf.as_deref().and_then(Shelf::parse).map(Action::File);
        let junk = m.junk.then_some(Action::Junk);
        [case, shelf, junk].into_iter().flatten().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_virtual_secretary_folders() {
        let mappings: Vec<FolderMapping> = toml::from_str::<std::collections::HashMap<String, Vec<FolderMapping>>>(
            r#"
            [[map]]
            folder = "INBOX.Money.Taxes"
            case = "taxes"
            [[map]]
            folder = "INBOX.Services.Notifications"
            shelf = "notifications"
            [[map]]
            folder = "INBOX.spam"
            junk = true
            "#,
        )
        .unwrap()
        .remove("map")
        .unwrap();
        assert_eq!(FolderMapping::actions_for(&mappings, "inbox.money.taxes"), vec![Action::AssignCase("taxes".into())]);
        assert_eq!(FolderMapping::actions_for(&mappings, "INBOX.spam"), vec![Action::Junk]);
        assert!(FolderMapping::actions_for(&mappings, "INBOX").is_empty());
    }
}

// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The Contacts page's filters, combined (wave D, 18 November 2026): by
//! address book (one, several or all), by group (one, or in none), whether a
//! card has or lacks a phone number, an e-mail address, a postal address or a
//! title (`contacts::has_title`), and its name containing a text (the search
//! field: the name, the organisation, the addresses and numbers, as
//! `contacts::search` reads them). Decided here, tested here; the page only
//! draws. Kept on this device, `$XDG_STATE_HOME/sioul/contacts-view.toml`, as
//! the other pages keep their view: never shared, an older Sioul ignores it.
//!
//! An address book filters what is shown, nothing more: who may reach you is
//! decided by a card's groups (porch.rs), never by its book (but for the
//! trust a book someone else can write needs, `addressbooks::groups_count`).

use crate::contacts::{self, Contact};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Has, lacks, or either.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Want {
    #[default]
    Any,
    Has,
    Lacks,
}

impl Want {
    fn is_any(&self) -> bool {
        *self == Want::Any
    }

    /// Whether a card with (`has`) or without it stays.
    fn keeps(self, has: bool) -> bool {
        match self {
            Want::Any => true,
            Want::Has => has,
            Want::Lacks => !has,
        }
    }
}

/// The filters, as chosen on this device.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Filter {
    /// The address books shown, by place ("account/id"); none: all of them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub books: Vec<String>,
    /// The group shown (a category, as written); empty: any.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub group: String,
    /// Only the cards in no group (`group` then empty).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub ungrouped: bool,
    #[serde(default, skip_serializing_if = "Want::is_any")]
    pub phone: Want,
    #[serde(default, skip_serializing_if = "Want::is_any")]
    pub email: Want,
    #[serde(default, skip_serializing_if = "Want::is_any")]
    pub address: Want,
    #[serde(default, skip_serializing_if = "Want::is_any")]
    pub title: Want,
    /// What the search field holds: for this run of the window only, never
    /// written to disk (review F23: the last name looked for is no one's business).
    #[serde(default, skip_serializing)]
    pub name: String,
}

impl Filter {
    /// `$XDG_STATE_HOME/sioul/contacts-view.toml`.
    pub fn default_path() -> PathBuf {
        crate::config::state_dir().join("contacts-view.toml")
    }

    /// As kept on this device; nothing kept, or unreadable: no filter.
    pub fn load(path: &Path) -> Filter {
        let mut kept: Filter = std::fs::read_to_string(path).ok().and_then(|t| toml::from_str(&t).ok()).unwrap_or_default();
        kept.name.clear();
        kept
    }

    /// Kept, the search left out, only when it changed: written beside, then moved in.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        if Filter::load(path) == (Filter { name: String::new(), ..self.clone() }) {
            return Ok(());
        }
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        let text = toml::to_string(self).map_err(|e| e.to_string())?;
        crate::filelock::replace(path, text.as_bytes()).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// Fitted to what exists now: books gone dropped (none left, all shown),
    /// a group no card has any more dropped (as the page's group filter fell
    /// back to all), the group's spelling the one in use.
    pub fn fitted(&self, places: &[String], groups: &[String]) -> Filter {
        let mut fitted = self.clone();
        fitted.books.retain(|b| places.contains(b));
        fitted.books.dedup();
        let wanted = contacts::category_key(&self.group);
        fitted.group = groups.iter().find(|g| !wanted.is_empty() && contacts::category_key(g) == wanted).cloned().unwrap_or_default();
        if !fitted.group.is_empty() {
            fitted.ungrouped = false;
        }
        fitted
    }

    /// Whether anything but the search narrows the list.
    pub fn narrows(&self) -> bool {
        !self.books.is_empty() || !self.group.is_empty() || self.ungrouped || [self.phone, self.email, self.address, self.title].iter().any(|w| !w.is_any())
    }

    /// Whether anything but the search and the group narrows the list.
    pub fn narrows_beyond_group(&self) -> bool {
        !self.books.is_empty() || self.ungrouped || [self.phone, self.email, self.address, self.title].iter().any(|w| !w.is_any())
    }

    /// Whether a card stays.
    pub fn keeps(&self, contact: &Contact) -> bool {
        (self.books.is_empty() || self.books.contains(&contact.place))
            && (self.group.is_empty() || contacts::in_category(contact, &self.group))
            && (!self.ungrouped || contact.categories.is_empty())
            && self.phone.keeps(!contact.phones.is_empty())
            && self.email.keeps(!contact.emails.is_empty())
            && self.address.keeps(!contact.addresses.is_empty())
            && self.title.keeps(contacts::has_title(contact))
    }

    /// The cards that stay, in their order.
    pub fn apply<'a>(&self, all: &'a [Contact]) -> Vec<&'a Contact> {
        contacts::search(all, &self.name).into_iter().filter(|c| self.keeps(c)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contacts::Labeled;

    fn card(name: &str, place: &str, groups: &[&str], phone: bool, email: bool, address: bool, prefix: &str) -> Contact {
        let one = |on: bool, value: &str| if on { vec![Labeled { label: String::new(), value: value.into() }] } else { Vec::new() };
        Contact {
            key: format!("/{place}/{name}.vcf"),
            name: name.into(),
            place: place.into(),
            categories: groups.iter().map(|g| g.to_string()).collect(),
            phones: one(phone, "+33 1 99 00 00 00"),
            emails: one(email, "someone@example.org"),
            addresses: one(address, "1 rue de l'Exemple\n69000 Lyon"),
            prefix: prefix.into(),
            ..Contact::default()
        }
    }

    fn names(filter: &Filter, all: &[Contact]) -> Vec<String> {
        filter.apply(all).into_iter().map(|c| c.name.clone()).collect()
    }

    /// Each filter alone, then combined: one book or several, a group or
    /// none, has or lacks, the name; what is gone falls back to all.
    #[test]
    fn each_filter_and_their_combinations() {
        let all = [
            card("Mme Jeanne Martin", "cloud/contacts", &["Clients"], true, true, false, ""),
            card("Paul Exemple", "cloud/contacts", &[], true, false, true, ""),
            card("Lou Exemple", "cloud/pro", &["Clients", "Fournisseurs"], false, true, true, "Dr"),
            card("Sam Exemple", "local/home", &["Famille"], false, false, false, ""),
        ];
        let none = Filter::default();
        assert_eq!(names(&none, &all).len(), 4);
        assert!(!none.narrows());
        let f = |change: &dyn Fn(&mut Filter)| {
            let mut filter = Filter::default();
            change(&mut filter);
            names(&filter, &all)
        };
        assert_eq!(f(&|x| x.books = vec!["cloud/contacts".into()]), ["Mme Jeanne Martin", "Paul Exemple"]);
        assert_eq!(f(&|x| x.books = vec!["cloud/pro".into(), "local/home".into()]), ["Lou Exemple", "Sam Exemple"]);
        assert_eq!(f(&|x| x.group = "clients".into()), ["Mme Jeanne Martin", "Lou Exemple"], "case aside");
        assert_eq!(f(&|x| x.ungrouped = true), ["Paul Exemple"]);
        assert_eq!(f(&|x| x.phone = Want::Has), ["Mme Jeanne Martin", "Paul Exemple"]);
        assert_eq!(f(&|x| x.phone = Want::Lacks), ["Lou Exemple", "Sam Exemple"]);
        assert_eq!(f(&|x| x.email = Want::Lacks), ["Paul Exemple", "Sam Exemple"]);
        assert_eq!(f(&|x| x.address = Want::Has), ["Paul Exemple", "Lou Exemple"]);
        assert_eq!(f(&|x| x.title = Want::Has), ["Mme Jeanne Martin", "Lou Exemple"], "at the start of the name, or in its field");
        assert_eq!(f(&|x| x.title = Want::Lacks), ["Paul Exemple", "Sam Exemple"]);
        assert_eq!(f(&|x| x.name = "exemple".into()), ["Paul Exemple", "Lou Exemple", "Sam Exemple"]);
        // Combined: every one must hold.
        assert_eq!(
            f(&|x| {
                x.group = "Clients".into();
                x.email = Want::Has;
                x.title = Want::Has;
                x.books = vec!["cloud/pro".into()];
            }),
            ["Lou Exemple"]
        );
        assert_eq!(
            f(&|x| {
                x.phone = Want::Has;
                x.address = Want::Lacks;
                x.name = "jeanne".into();
            }),
            ["Mme Jeanne Martin"]
        );
        assert!(f(&|x| {
            x.ungrouped = true;
            x.phone = Want::Lacks;
        })
        .is_empty());
        // Fitted to what exists: a book gone and a group gone fall back to all.
        let kept = Filter { books: vec!["gone/book".into(), "cloud/pro".into()], group: "CLIENTS".into(), ..Filter::default() };
        let fitted = kept.fitted(&["cloud/pro".into(), "cloud/contacts".into()], &["Clients".into()]);
        assert_eq!((fitted.books.clone(), fitted.group.as_str()), (vec!["cloud/pro".to_string()], "Clients"));
        let gone = Filter { books: vec!["gone/book".into()], group: "Voisins".into(), ..Filter::default() }.fitted(&["cloud/pro".into()], &["Clients".into()]);
        assert!(!gone.narrows() && names(&gone, &all).len() == 4);
    }

    /// Kept on this device, written only when it changed; an unreadable file is no filter.
    #[test]
    fn kept_on_this_device() {
        let dir = std::env::temp_dir().join(format!("sioul-contact-filter-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("contacts-view.toml");
        assert_eq!(Filter::load(&path), Filter::default());
        let filter = Filter { books: vec!["cloud/pro".into()], title: Want::Has, phone: Want::Lacks, name: "Martin".into(), ..Filter::default() };
        filter.save(&path).unwrap();
        assert_eq!(Filter::load(&path), Filter { name: String::new(), ..filter.clone() }, "the search is never kept");
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("title = \"has\"") && text.contains("phone = \"lacks\"") && !text.contains("email") && !text.contains("Martin"), "{text}");
        std::fs::write(&path, "books = 12").unwrap();
        assert_eq!(Filter::load(&path), Filter::default());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

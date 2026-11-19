// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! "Export…" (wave D, 18 November 2026): an address book's cards, or the
//! cards selected on the Contacts page, written as one vCard file where you
//! choose: the copy to keep before a large batch, and the way to hand
//! contacts to another program. The cards go as their files hold them
//! (`vdir::cards_as_vcf`); nothing is sent anywhere.

use crate::backend::{QtThread, Shared, say, tell, tr};
use serde::Deserialize;
use sioul_core::vdir::{self, Kind};
use std::sync::Arc;

#[derive(Deserialize, Default)]
struct Asked {
    /// A book's place, "account/id"; else the cards named by `keys`.
    #[serde(default)]
    book: String,
    #[serde(default)]
    keys: Vec<String>,
}

/// The cards asked (`text`, JSON {book} or {keys}) written as one vCard file
/// at `url` (a file's address, as the save dialog gives it). "" done (said
/// in the status line), else why not.
pub(crate) fn export(qt: &QtThread, shared: &Arc<Shared>, text: &str, url: &str) -> String {
    let asked: Asked = serde_json::from_str(text).unwrap_or_default();
    let files: Vec<std::path::PathBuf> = if asked.book.is_empty() {
        asked.keys.iter().filter_map(|k| crate::pim::ours(k)).filter(|p| p.starts_with(Kind::Contacts.root())).collect()
    } else {
        match sioul_core::addressbooks::find(&asked.book) {
            Some(book) => book.items(),
            None => return tr().text("books-gone", None),
        }
    };
    if url.starts_with("content:") {
        return tr().text("books-export-on-a-phone", None);
    }
    let path = crate::backend::local_path(url);
    // Never into Sioul's own address books, where it would be read as a card.
    if path.as_os_str().is_empty() || path.starts_with(Kind::Contacts.root()) {
        return tr().text("books-export-where", None);
    }
    let text = vdir::cards_as_vcf(&files);
    let count = text.matches("BEGIN:VCARD").count();
    if let Err(e) = sioul_core::filelock::replace(&path, text.as_bytes()) {
        return format!("{}: {e}", path.display());
    }
    tell(qt, shared, say("books-exported", &[("n", count.to_string()), ("path", path.display().to_string())]));
    String::new()
}

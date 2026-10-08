// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! A text drafted by an AI agent (`sioul mcp`, `draft_text`; docs/mcp.md,
//! "Texts"): it waits in the Texts page, in its conversation, until you use
//! it (its words go in the box, and sending stays your Send) or discard it.
//! **Never sent**: a draft is no request (`texts::Request`), and nothing
//! reads this file to send.
//!
//! Kept on this device only, never shared: `drafts.jsonl` in the texts'
//! private folder (`texts::private_folder`), each line sealed as the texts'
//! own lines are (`texts::Sealer`), so that a draft is unreadable without the
//! key, as the texts it answers are.

use crate::texts::{self, Sealer};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// The file's name in the texts' private folder.
pub const FILE: &str = "drafts.jsonl";

/// One draft waiting.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextDraft {
    /// Random, 32 hex digits (`texts::new_key`).
    pub id: String,
    /// The number, as written.
    pub to: String,
    /// Its conversation (`texts::conversation_id` of the number's key), where the page shows it.
    pub conversation: String,
    pub body: String,
    /// When it was written (ms).
    pub written: i64,
    /// Who wrote it: the agent's name, as its client gave it.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub by: String,
}

/// Where the drafts are kept.
pub fn path() -> PathBuf {
    texts::private_folder().join(FILE)
}

/// Every draft of the file that opens, oldest first.
pub fn all_in(file: &Path, sealer: &dyn Sealer) -> Vec<TextDraft> {
    let mut drafts: Vec<TextDraft> = texts::read_file(file, sealer);
    drafts.sort_by_key(|d| d.written);
    drafts
}

pub fn all(sealer: &dyn Sealer) -> Vec<TextDraft> {
    all_in(&path(), sealer)
}

/// A draft added, its words sealed; never over another (a new id).
pub fn add_in(file: &Path, draft: &TextDraft, sealer: &dyn Sealer) -> Result<(), String> {
    texts::append(file, std::slice::from_ref(draft), sealer, Some(&|d: &TextDraft| d.id.clone())).map(|_| ())
}

pub fn add(draft: &TextDraft, sealer: &dyn Sealer) -> Result<(), String> {
    add_in(&path(), draft, sealer)
}

/// A draft taken out (used, or discarded): the file written again without
/// it, under its lock, private, beside then put in place. Lines that do not
/// open stay as they were. Whether it was there.
pub fn remove_in(file: &Path, id: &str, sealer: &dyn Sealer) -> Result<bool, String> {
    let gone = crate::calls::rewrite_lines(file, |line| sealer.open(line.trim()).and_then(|plain| serde_json::from_str::<TextDraft>(&plain).ok()).is_some_and(|d| d.id == id))?;
    Ok(gone > 0)
}

pub fn remove(id: &str, sealer: &dyn Sealer) -> Result<bool, String> {
    remove_in(&path(), id, sealer)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A sealer for tests: reversible, and visibly not plain text.
    struct Rot;
    impl Sealer for Rot {
        fn seal(&self, plain: &str) -> String {
            format!("rot:{}", plain.chars().rev().collect::<String>())
        }
        fn open(&self, sealed: &str) -> Option<String> {
            sealed.strip_prefix("rot:").map(|s| s.chars().rev().collect())
        }
    }

    #[test]
    fn drafts_wait_sealed_until_used_or_discarded() {
        let dir = std::env::temp_dir().join(format!("sioul-textdraft-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let file = dir.join(FILE);
        let one = TextDraft { id: "a".repeat(32), to: "01 99 00 12 34".into(), conversation: "+33199001234".into(), body: "See you at six.".into(), written: 2, by: "claude-code".into() };
        let two = TextDraft { id: "b".repeat(32), written: 1, ..one.clone() };
        add_in(&file, &one, &Rot).unwrap();
        add_in(&file, &two, &Rot).unwrap();
        // The same draft twice is kept once.
        add_in(&file, &one, &Rot).unwrap();
        assert!(!std::fs::read_to_string(&file).unwrap().contains("See you at six"), "sealed at rest");
        assert_eq!(all_in(&file, &Rot).iter().map(|d| d.written).collect::<Vec<_>>(), [1, 2]);
        assert!(remove_in(&file, &one.id, &Rot).unwrap());
        assert!(!remove_in(&file, &one.id, &Rot).unwrap());
        assert_eq!(all_in(&file, &Rot), vec![two]);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

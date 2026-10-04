// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! What each agent asked, and what it was given (docs/ai.md, "A log"): one
//! JSON line per call, one file per month, in the state folder
//! (`$XDG_STATE_HOME/sioul/mcp/2026-10.jsonl`), readable by you alone. It
//! stays on this computer. What was handed over is named by its addresses
//! (mid:…, sioul:…) and its length, never by its words: no message's text, no
//! code, no secret is kept a second time, in a file that outlives the mail.

use super::mask;
use super::tools::Answer;
use jiff::Zoned;
use serde_json::{Map, Value, json};
use sioul_core::config;
use std::io::Write;
use std::path::Path;

/// Characters of an argument kept: a query, a title, an address.
const KEPT: usize = 200;

/// Appends one call to this month's file. A log that cannot be written is
/// said on standard error and never stops the call.
pub fn record(client: &str, tool: &str, arguments: &Value, outcome: &Result<Answer, String>) {
    let now = Zoned::now();
    let (error, given, characters) = match outcome {
        Ok(answer) => (Value::Null, addresses(&answer.text), answer.text.chars().count()),
        Err(message) => (Value::String(short(message)), Vec::new(), 0),
    };
    let line = json!({
        "at": now.strftime("%Y-%m-%dT%H:%M:%S%:z").to_string(), "client": client, "tool": tool,
        "arguments": kept(arguments, 0), "error": error, "given": given, "characters": characters,
    });
    let folder = config::state_dir().join("mcp");
    let path = folder.join(format!("{}.jsonl", now.strftime("%Y-%m")));
    // One write per line: two calls logged at once never mix their lines.
    let written = private_folder(&folder).and_then(|()| open(&path)).and_then(|mut file| file.write_all(format!("{line}\n").as_bytes()));
    if let Err(e) = written {
        eprintln!("{}: {e}", path.display());
    }
}

/// An argument as it is kept: a body or notes by their length alone (a draft
/// or a note may quote mail), any other text shortened, its numbers masked.
fn kept(value: &Value, depth: usize) -> Value {
    match value {
        Value::String(text) => Value::String(short(text)),
        Value::Array(items) => Value::Array(items.iter().take(50).map(|item| kept(item, depth + 1)).collect()),
        Value::Object(map) if depth == 0 => Value::Object(
            map.iter()
                .map(|(key, value)| {
                    let value = match (key.as_str(), value) {
                        ("body" | "notes", Value::String(text)) => json!(format!("({} characters)", text.chars().count())),
                        _ => kept(value, depth + 1),
                    };
                    (crate::one_line(key).chars().take(KEPT).collect(), value)
                })
                .collect::<Map<String, Value>>(),
        ),
        Value::Object(_) => json!("(an object)"),
        other => other.clone(),
    }
}

/// A text shortened to what tells what it was, on one line, its account and card numbers masked.
fn short(text: &str) -> String {
    let text = mask::text(&crate::one_line(text));
    let count = text.chars().count();
    if count <= KEPT { text } else { format!("{}… ({count} characters)", text.chars().take(KEPT).collect::<String>()) }
}

/// The addresses an answer gave, each once: what the agent was shown, by name.
fn addresses(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let words = text.split(|c: char| c.is_whitespace() || matches!(c, '<' | '>' | '[' | ']' | '(' | ')' | '"' | ','));
    for word in words.filter(|w| (w.starts_with("mid:") || w.starts_with("sioul:")) && w.len() > 6) {
        if out.len() < 500 && !out.iter().any(|seen| seen == word) {
            out.push(word.chars().take(KEPT).collect());
        }
    }
    out
}

/// The log's folder, made for you alone where the system has owners (Unix).
fn private_folder(folder: &Path) -> std::io::Result<()> {
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder.create(folder)
}

/// This month's file, to add to; readable and writable by you alone on Unix
/// (on Windows, your profile's folders are yours already).
fn open(path: &Path) -> std::io::Result<std::fs::File> {
    let mut options = std::fs::OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let file = options.open(path)?;
    // A file made before, by a version that left it readable to others: closed now.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if file.metadata()?.permissions().mode() & 0o077 != 0 {
            file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
        }
    }
    Ok(file)
}

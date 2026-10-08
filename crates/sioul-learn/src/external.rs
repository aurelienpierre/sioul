// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Outside training material: mail labelled elsewhere (an older filter's
//! dump, another mail client's archive, a public corpus), imported once to
//! give the training more to learn from, and a baseline to measure it
//! against (docs/spam-filter.md, "Outside material"). `sioul spam import
//! <file>` reads it; `sioul spam import --remove <source>` takes it away.
//!
//! The format: JSON lines, one message each, a plain file or gzipped:
//!
//! ```text
//! {"label":"spam","date":"2023-06-20T16:23:00+02:00","subject":"You won","text":"Claim your prize…","source":"old-filter"}
//! {"label":"ham","date":1687271000,"subject":"Lunch","text":"Tomorrow?","from":"Jane <jane@example.org>"}
//! ```
//!
//! - `label`: "spam" or "ham" (required);
//! - `date`: when it arrived, RFC 3339 or Unix seconds (required): the
//!   training's split is by time;
//! - `subject`, `text`: its subject and its text part (required; one of them,
//!   or `html`, not empty);
//! - `html`: its HTML, read as text as the Porch reads it when there is no
//!   text, or only a stand-in ("view it in your browser");
//! - `from`: its sender; `headers`: its raw header block, of which only the
//!   Message-ID is read now (a message also in your own mail is learned once,
//!   from yours); both are kept for a later version;
//! - `source`: a name for where it comes from; else the one given to the
//!   import, else the file's name.
//!
//! Kept apart from your own corpus, never shared; an AI agent sees of it
//! only the sender and the subject, masked, of a test's worst errors, when
//! you let it use the spam filter's tools (`[mcp] spam`, docs/mcp.md):
//! `$XDG_DATA_HOME/sioul/spam/external/<source>.jsonl.gz`, each
//! message checked and cut to what the Porch reads (6 000 characters of
//! text, 64 KB of HTML, 32 KB of headers), with `<source>.toml` beside it
//! (counts only). An import replaces what was imported before under the same
//! name.
//!
//! In training (`train::train`): its words join the language model's
//! corpus, and its messages the classifier's learning set with their header
//! features at the training's mean (0 once standardized): they pull no
//! header weight, only the words learn from them. Each source is split in
//! time as your own mail is (its oldest 80 % learned from, its newest 20 %
//! held out); the held-out part gives "the baseline", the same numbers as
//! for your own mail, said apart.

use crate::corpus::{HEADER_BYTES, HTML_BYTES, Leaf, Node, Text};
use crate::labels::Label;
use crate::{Dirs, LearnError, io_error, now};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

/// The text kept of a message, in characters: what the Porch reads.
const TEXT_CHARS: usize = 6000;
/// The subject kept, in characters.
const SUBJECT_CHARS: usize = 1000;
/// A source's name when none is given anywhere.
const UNNAMED: &str = "outside";

/// One message as imported: checked, its date in Unix seconds, its texts cut
/// to what the Porch reads. Its source is its file's name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    pub label: Label,
    /// When it arrived, Unix seconds.
    pub date: i64,
    pub subject: String,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub html: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headers: Option<String>,
}

impl Message {
    /// Its Message-ID, bare, from its headers when it came with them.
    pub fn message_id(&self) -> Option<String> {
        crate::labels::message_id(self.headers.as_deref()?.as_bytes())
    }

    /// Its words, as the Porch reads a message: the subject, the text, the
    /// HTML read as text when there is no text or only a stand-in; read with
    /// `lexicon` (a training's, or the table's own).
    pub fn words(&self, lexicon: &crate::spamcore::Lexicon) -> Vec<String> {
        let excerpt = self.rebuilt(b"").map(|card| card.excerpt).unwrap_or_else(|| self.text.clone());
        crate::spamcore::tokens_with(lexicon, &self.subject, &excerpt).words.into_iter().filter(|w| !w.is_empty() && !w.contains(char::is_whitespace)).collect()
    }

    /// The message as the Porch would read it: its sender, its subject, its
    /// Message-ID and the start of its text, so that a test's worst errors
    /// can say which message each is (`detail`). Each field on one line.
    pub fn card(&self) -> Option<sioul_core::card::Card> {
        let line = |text: &str| text.replace(['\r', '\n'], " ");
        let mut header = String::new();
        if let Some(from) = self.from.as_deref() {
            header.push_str(&format!("From: {}\r\n", line(from)));
        }
        header.push_str(&format!("Subject: {}\r\n", line(&self.subject)));
        if let Some(id) = self.message_id() {
            header.push_str(&format!("Message-ID: <{}>\r\n", line(&id)));
        }
        self.rebuilt(header.as_bytes())
    }

    /// The message made again from its texts, as a corpus record is (`spamcore::rebuild`), under `header`.
    fn rebuilt(&self, header: &[u8]) -> Option<sioul_core::card::Card> {
        let plain = (!self.text.is_empty()).then(|| Text { at: vec![1], text: self.text.clone() });
        let html = self.html.as_ref().filter(|h| !h.is_empty()).map(|h| Text { at: vec![if plain.is_some() { 2 } else { 1 }], text: h.clone() });
        let leaf = |mime: &str| Node::Leaf(Leaf { mime: mime.into(), charset: Some("utf-8".into()), ..Leaf::default() });
        let structure = match (&plain, &html) {
            (Some(_), Some(_)) => Node::Multipart { subtype: "alternative".into(), parts: vec![leaf("text/plain"), leaf("text/html")] },
            (None, Some(_)) => leaf("text/html"),
            _ => leaf("text/plain"),
        };
        let raw = crate::spamcore::rebuild(header, Some(&structure), plain.as_ref(), html.as_ref());
        sioul_core::card::Card::from_bytes(&raw)
    }
}

/// One line as written by whoever made the file, before it is checked.
#[derive(Debug, Deserialize)]
struct Line {
    label: Option<String>,
    date: Option<serde_json::Value>,
    subject: Option<String>,
    text: Option<String>,
    #[serde(default)]
    html: Option<String>,
    #[serde(default)]
    from: Option<String>,
    #[serde(default)]
    headers: Option<String>,
    #[serde(default)]
    source: Option<String>,
}

/// Why a line was left out: said by these words, counted.
pub const REFUSED: [&str; 6] = ["not-json", "label", "date", "fields", "empty", "copy"];

/// What one import did: counts only, never a message.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Imported {
    /// Lines read.
    pub lines: u64,
    /// Each source written, with what it now holds.
    pub sources: BTreeMap<String, Counts>,
    /// Lines left out, by why (`REFUSED`).
    pub refused: BTreeMap<&'static str, u64>,
}

/// What a source holds: counts and dates only.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Counts {
    pub ham: u64,
    pub spam: u64,
    /// Its oldest and newest message's dates (Unix seconds).
    pub first: i64,
    pub last: i64,
    /// When it was imported (Unix seconds).
    #[serde(default)]
    pub imported: i64,
}

impl Counts {
    fn add(&mut self, message: &Message) {
        match message.label {
            Label::Ham => self.ham += 1,
            Label::Spam => self.spam += 1,
        }
        self.first = if self.ham + self.spam == 1 { message.date } else { self.first.min(message.date) };
        self.last = self.last.max(message.date);
    }
}

/// A source as `sioul spam status` and the settings say it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    pub name: String,
    pub counts: Counts,
    /// The bytes its file takes.
    pub bytes: u64,
}

/// A source's name, safe as a file's: lowercase letters, digits and dashes.
pub fn source_name(text: &str) -> String {
    let name = sioul_core::config::slug(text);
    if name.is_empty() { UNNAMED.to_string() } else { name }
}

fn file_of(dirs: &Dirs, source: &str) -> PathBuf {
    dirs.external().join(format!("{source}.jsonl.gz"))
}

fn counts_of(dirs: &Dirs, source: &str) -> PathBuf {
    dirs.external().join(format!("{source}.toml"))
}

/// A date as the format allows it: Unix seconds (a number, or digits), or RFC 3339.
fn date_of(value: &serde_json::Value) -> Option<i64> {
    match value {
        serde_json::Value::Number(n) => n.as_i64().or_else(|| n.as_f64().filter(|f| f.is_finite()).map(|f| f as i64)),
        serde_json::Value::String(text) => {
            let text = text.trim();
            text.parse::<i64>().ok().or_else(|| text.parse::<jiff::Timestamp>().ok().map(|t| t.as_second()))
        }
        _ => None,
    }
}

/// The first `chars` characters of a text.
fn cut_chars(text: &str, chars: usize) -> String {
    text.chars().take(chars).collect()
}

/// The first `bytes` bytes of a text, cut on a character's edge.
fn cut_bytes(text: &str, bytes: usize) -> String {
    let mut end = text.len().min(bytes);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_string()
}

/// A line checked: its source's name and the message; or why it is left out.
fn check(line: &str, default_source: &str) -> Result<(String, Message), &'static str> {
    let line: Line = serde_json::from_str(line).map_err(|_| "not-json")?;
    let label = match line.label.as_deref().map(|l| l.trim().to_ascii_lowercase()).as_deref() {
        Some("spam") => Label::Spam,
        Some("ham") => Label::Ham,
        _ => return Err("label"),
    };
    let date = line.date.as_ref().and_then(date_of).ok_or("date")?;
    let (Some(subject), Some(text)) = (line.subject, line.text) else { return Err("fields") };
    let html = line.html.filter(|h| !h.trim().is_empty());
    if subject.trim().is_empty() && text.trim().is_empty() && html.is_none() {
        return Err("empty");
    }
    let source = line.source.as_deref().map(source_name).unwrap_or_else(|| default_source.to_string());
    Ok((
        source,
        Message {
            label,
            date,
            subject: cut_chars(&subject, SUBJECT_CHARS),
            text: cut_chars(&text, TEXT_CHARS),
            html: html.map(|h| cut_bytes(&h, HTML_BYTES as usize)),
            from: line.from.filter(|f| !f.trim().is_empty()),
            headers: line.headers.filter(|h| !h.trim().is_empty()).map(|h| cut_bytes(&h, HEADER_BYTES as usize)),
        },
    ))
}

/// What tells one message from another within a source: its Message-ID, else its date, subject and text.
fn key_of(message: &Message) -> u64 {
    match message.message_id() {
        Some(id) => crate::labels::fnv64(id.as_bytes()),
        None => crate::labels::fnv64(format!("{}\u{1f}{}\u{1f}{}", message.date, message.subject, message.text).as_bytes()),
    }
}

/// Imports a file of outside material (JSON lines, gzipped or not): each
/// source it names replaces what was kept before under that name. A line's
/// own `source` first, else `source`, else the file's name.
pub fn import(dirs: &Dirs, path: &Path, source: Option<&str>) -> Result<Imported, LearnError> {
    let file = std::fs::File::open(path).map_err(|e| io_error(path, e))?;
    let mut reader = BufReader::new(file);
    let gzipped = reader.fill_buf().map_err(|e| io_error(path, e))?.starts_with(&[0x1f, 0x8b]);
    let stem = path.file_name().and_then(|n| n.to_str()).map(|n| n.split('.').next().unwrap_or(n)).unwrap_or(UNNAMED);
    let default_source = source_name(source.unwrap_or(stem));
    if gzipped {
        import_from(dirs, BufReader::new(flate2::read::MultiGzDecoder::new(reader)), &default_source)
    } else {
        import_from(dirs, reader, &default_source)
    }
}

/// A source being written: its file beside its place, its counts, what it holds already.
struct Writing {
    temporary: PathBuf,
    out: flate2::write::GzEncoder<std::fs::File>,
    counts: Counts,
    seen: HashSet<u64>,
}

/// Imports lines from `reader` (see `import`).
pub fn import_from(dirs: &Dirs, reader: impl BufRead, default_source: &str) -> Result<Imported, LearnError> {
    let folder = dirs.external();
    make_private(&folder)?;
    let mut imported = Imported::default();
    let mut writing: BTreeMap<String, Writing> = BTreeMap::new();
    for line in reader.split(b'\n') {
        let line = line.map_err(|e| io_error(&folder, e))?;
        let line = String::from_utf8_lossy(&line);
        if line.trim().is_empty() {
            continue;
        }
        imported.lines += 1;
        let (source, message) = match check(&line, default_source) {
            Ok(checked) => checked,
            Err(why) => {
                *imported.refused.entry(why).or_default() += 1;
                continue;
            }
        };
        if !writing.contains_key(&source) {
            let temporary = folder.join(format!(".{source}.jsonl.gz.{}", std::process::id()));
            let file = private_file(&temporary)?;
            writing.insert(source.clone(), Writing { temporary, out: flate2::write::GzEncoder::new(file, flate2::Compression::default()), counts: Counts::default(), seen: HashSet::new() });
        }
        let Some(w) = writing.get_mut(&source) else { continue };
        if !w.seen.insert(key_of(&message)) {
            *imported.refused.entry("copy").or_default() += 1;
            continue;
        }
        let mut text = serde_json::to_vec(&message).map_err(|e| io_error(&w.temporary, e))?;
        text.push(b'\n');
        w.out.write_all(&text).map_err(|e| io_error(&w.temporary, e))?;
        w.counts.add(&message);
    }
    for (source, w) in writing {
        let Writing { temporary, out, mut counts, .. } = w;
        let file = out.finish().map_err(|e| io_error(&temporary, e))?;
        file.sync_all().map_err(|e| io_error(&temporary, e))?;
        drop(file);
        counts.imported = now();
        std::fs::rename(&temporary, file_of(dirs, &source)).map_err(|e| io_error(&temporary, e))?;
        // Its counts beside it, written whole, yours alone as it is.
        let text = toml::to_string(&counts).map_err(|e| io_error(&folder, e))?;
        let beside = folder.join(format!(".{source}.toml.{}", std::process::id()));
        private_file(&beside)?.write_all(text.as_bytes()).map_err(|e| io_error(&beside, e))?;
        std::fs::rename(&beside, counts_of(dirs, &source)).map_err(|e| io_error(&beside, e))?;
        imported.sources.insert(source, counts);
    }
    Ok(imported)
}

/// The folder made, yours alone (Unix).
fn make_private(folder: &Path) -> Result<(), LearnError> {
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder.create(folder).map_err(|e| io_error(folder, e))
}

/// A new file, yours alone (Unix).
fn private_file(path: &Path) -> Result<std::fs::File, LearnError> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options.open(path).map_err(|e| io_error(path, e))
}

/// Takes a source away: its file and its counts. False when there was none.
pub fn remove(dirs: &Dirs, source: &str) -> Result<bool, LearnError> {
    let source = source_name(source);
    let file = file_of(dirs, &source);
    let found = file.exists();
    for path in [file, counts_of(dirs, &source)] {
        match std::fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(io_error(&path, e)),
            _ => {}
        }
    }
    Ok(found)
}

/// The sources kept, by name: what each holds, from the counts written at its import.
pub fn sources(dirs: &Dirs) -> Vec<Source> {
    let mut found: Vec<Source> = std::fs::read_dir(dirs.external())
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().to_str()?.strip_suffix(".jsonl.gz")?.to_string();
            (!name.starts_with('.')).then_some(name)
        })
        .map(|name| {
            let counts = std::fs::read_to_string(counts_of(dirs, &name)).ok().and_then(|t| toml::from_str(&t).ok()).unwrap_or_default();
            let bytes = std::fs::metadata(file_of(dirs, &name)).map_or(0, |m| m.len());
            Source { name, counts, bytes }
        })
        .collect();
    found.sort_by(|a, b| a.name.cmp(&b.name));
    found
}

/// Every message of every source, source after source (by name), in the order imported.
pub fn read_all(dirs: &Dirs, mut each: impl FnMut(&str, Message)) -> Result<(), LearnError> {
    for source in sources(dirs) {
        let path = file_of(dirs, &source.name);
        let file = std::fs::File::open(&path).map_err(|e| io_error(&path, e))?;
        let reader = BufReader::new(flate2::read::MultiGzDecoder::new(BufReader::new(file)));
        for line in reader.split(b'\n') {
            let Ok(line) = line else { break };
            if let Ok(message) = serde_json::from_slice::<Message>(&line) {
                each(&source.name, message);
            }
        }
    }
    Ok(())
}

/// The lines of a file of outside material, read whole (tests).
#[cfg(test)]
fn read_source(dirs: &Dirs, source: &str) -> Vec<Message> {
    use std::io::Read;
    let mut text = String::new();
    let file = std::fs::File::open(file_of(dirs, source)).unwrap();
    flate2::read::MultiGzDecoder::new(file).read_to_string(&mut text).unwrap();
    text.lines().map(|l| serde_json::from_str(l).unwrap()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sioul-learn-external-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// The format as docs/spam-filter.md says it: what is kept, what is left out and why.
    #[test]
    fn lines_are_checked_and_kept_apart() {
        let root = scratch("lines");
        let dirs = Dirs::under(&root);
        let file = root.join("old-filter.jsonl");
        let long = "word ".repeat(3000);
        let lines = [
            r#"{"label":"spam","date":"2023-06-20T16:23:00+02:00","subject":"You won","text":"Claim your prize","source":"Old Filter"}"#.to_string(),
            r#"{"label":"HAM","date":1687271000,"subject":"Lunch","text":"Tomorrow?","from":"Jane <jane@example.org>"}"#.to_string(),
            r#"{"label":"ham","date":"1687271060","subject":"Notes","text":"","html":"<p>The notes</p>","headers":"Message-ID: <n1@example.org>\r\nFrom: a@example.org\r\n"}"#.to_string(),
            format!(r#"{{"label":"ham","date":1687271120,"subject":"Long","text":"{long}"}}"#),
            r#"{"label":"maybe","date":1,"subject":"x","text":"y"}"#.to_string(),
            r#"{"label":"spam","date":"yesterday","subject":"x","text":"y"}"#.to_string(),
            r#"{"label":"spam","date":1,"text":"no subject field"}"#.to_string(),
            r#"{"label":"spam","date":1,"subject":" ","text":""}"#.to_string(),
            "not json".to_string(),
            r#"{"label":"ham","date":1687271000,"subject":"Lunch","text":"Tomorrow?"}"#.to_string(),
            String::new(),
        ];
        std::fs::write(&file, lines.join("\n")).unwrap();
        let imported = import(&dirs, &file, None).unwrap();
        assert_eq!(imported.lines, 10);
        assert_eq!(imported.refused, BTreeMap::from([("label", 1), ("date", 1), ("fields", 1), ("empty", 1), ("not-json", 1), ("copy", 1)]));
        assert_eq!(imported.sources.keys().collect::<Vec<_>>(), ["old-filter"], "a line's own name, made safe; the file's name else");
        let counts = imported.sources["old-filter"];
        assert_eq!((counts.ham, counts.spam, counts.first, counts.last), (3, 1, 1_687_270_980, 1_687_271_120));
        let kept = read_source(&dirs, "old-filter");
        assert_eq!(kept[0].date, 1_687_270_980);
        assert_eq!(kept[3].text.chars().count(), TEXT_CHARS, "cut to what the Porch reads");
        assert_eq!(kept[2].message_id().as_deref(), Some("n1@example.org"));
        // Its words, as the Porch reads them: the HTML read as text when there is no text.
        let words = kept[2].words(&crate::spamcore::Lexicon::builtin());
        assert!(words.iter().any(|w| w.starts_with("note")), "{words:?}");
        assert_eq!(sources(&dirs).iter().map(|s| (s.name.as_str(), s.counts.ham, s.counts.spam)).collect::<Vec<_>>(), [("old-filter", 3, 1)]);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(dirs.external()).unwrap().permissions().mode() & 0o777, 0o700, "yours alone");
            assert_eq!(std::fs::metadata(file_of(&dirs, "old-filter")).unwrap().permissions().mode() & 0o777, 0o600);
            assert_eq!(std::fs::metadata(counts_of(&dirs, "old-filter")).unwrap().permissions().mode() & 0o777, 0o600);
        }
        // Imported again, gzipped, under another name given: a source of its own; the first one replaced when named alike.
        let gz = root.join("again.jsonl.gz");
        let mut encoder = flate2::write::GzEncoder::new(std::fs::File::create(&gz).unwrap(), flate2::Compression::default());
        encoder.write_all(lines[1].as_bytes()).unwrap();
        encoder.finish().unwrap();
        assert_eq!(import(&dirs, &gz, Some("old-filter")).unwrap().sources["old-filter"].ham, 1);
        assert_eq!(read_source(&dirs, "old-filter").len(), 1, "replaced");
        let mut all = Vec::new();
        read_all(&dirs, |source, m| all.push((source.to_string(), m.label))).unwrap();
        assert_eq!(all, [("old-filter".to_string(), Label::Ham)]);
        // Removed: nothing left of it.
        assert!(remove(&dirs, "Old Filter").unwrap());
        assert!(sources(&dirs).is_empty() && !remove(&dirs, "old-filter").unwrap());
        assert!(std::fs::read_dir(dirs.external()).unwrap().next().is_none());
        let _ = std::fs::remove_dir_all(root);
    }
}

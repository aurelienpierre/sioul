// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Notes: a folder of Markdown files, read the way Obsidian reads a vault.
//!
//! - **Links between notes**: `[[wikilinks]]` (`[[note]]`, `[[note#heading]]`,
//!   `[[note|shown as]]`, found by name anywhere in the vault) and ordinary
//!   Markdown links (`[text](../admin/letters.md#2)`, relative to the note).
//! - **Links to everything else**: `mid:` for a message (RFC 2392), `sioul:`
//!   for a task, an event, a contact, a draft (`links`), web addresses, files.
//! - **Links that come back**: every note that links to this one.
//! - **Tags**: `#tag` in the text, and `tags:` in the front matter.
//! - **Front matter**: the YAML block at the top, read for `title`, `tags`,
//!   `aliases` and links (`task:`, `event:`, `mail:` …).
//! - **Checkboxes**: `- [ ] line`, ready to become tasks.
//!
//! The folder is yours (the case store, docs/case-store.md). Sioul reads it,
//! and writes only the notes you write or make from it.

use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Folders never read: tools' own, and hidden ones.
const SKIPPED: &[&str] = &["node_modules", "target", ".git", ".obsidian", ".trash"];

/// A link found in a note.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NoteLink {
    /// Another note: "sioul:note/<path>"; else the address as written ("mid:…", "sioul:task/…", "https://…").
    pub target: String,
    /// "#heading" in the target, without the "#".
    pub anchor: String,
    /// The link's text.
    pub text: String,
    /// Its line, from 1.
    pub line: usize,
}

/// A checkbox line: `- [ ] call the CAF`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Checkbox {
    pub line: usize,
    pub done: bool,
    pub text: String,
}

/// What a file of the vault is: a Markdown note, or a picture, a PDF, a
/// sound (an audio memo) kept beside the notes, read here and linked the same way.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum NoteKind {
    #[default]
    Text,
    Image,
    Pdf,
    Audio,
}

/// The kind of a file by its extension; None for files the vault leaves out.
/// Text notes are Markdown, whether named `.md` (Obsidian's way) or `.txt`
/// (Nextcloud Notes' default, Markdown all the same).
pub fn kind_of_file(name: &str) -> Option<NoteKind> {
    let extension = name.rsplit_once('.')?.1.to_lowercase();
    Some(match extension.as_str() {
        "md" | "txt" => NoteKind::Text,
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "bmp" => NoteKind::Image,
        "pdf" => NoteKind::Pdf,
        "mp3" | "ogg" | "oga" | "opus" | "m4a" | "aac" | "wav" | "flac" | "webm" | "3gp" | "amr" => NoteKind::Audio,
        _ => return None,
    })
}

/// A text note's name without its extension: "letters.md" and "letters.txt"
/// both give "letters"; any other name is given back as it is.
pub fn text_stem(name: &str) -> &str {
    for extension in [".md", ".txt"] {
        let cut = name.len().saturating_sub(extension.len());
        if name.len() > extension.len() && name.is_char_boundary(cut) && name[cut..].eq_ignore_ascii_case(extension) {
            return &name[..cut];
        }
    }
    name
}

/// One note, as the index knows it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Note {
    /// Its place in the vault, with "/": "admin/letters.md".
    pub path: String,
    /// Markdown, or a picture, a PDF, a sound.
    pub kind: NoteKind,
    /// The front matter's title, else the first heading, else the file's name.
    pub title: String,
    pub tags: Vec<String>,
    pub aliases: Vec<String>,
    pub links: Vec<NoteLink>,
    pub checkboxes: Vec<Checkbox>,
    /// The front matter, key by key, each value as a list.
    pub front: BTreeMap<String, Vec<String>>,
    /// Last changed, Unix seconds.
    pub modified: i64,
}

impl Note {
    /// Its address for links: "sioul:note/admin/letters.md".
    pub fn uri(&self) -> String {
        uri_of(&self.path)
    }

    /// Its file name without ".md" or ".txt".
    pub fn stem(&self) -> &str {
        let name = self.path.rsplit('/').next().unwrap_or(&self.path);
        text_stem(name)
    }

    /// Its folder in the vault, "" at the root.
    pub fn folder(&self) -> &str {
        self.path.rsplit_once('/').map_or("", |(folder, _)| folder)
    }
}

/// Moves a note (or a picture, a PDF, a sound) into the vault's `.trash`,
/// as Obsidian does: out of sight, never lost. Returns where it went.
pub fn trash(root: &Path, path: &str) -> Result<String, String> {
    let path = normalize(path).ok_or_else(|| format!("{path}: outside the notes"))?;
    let mut target = format!(".trash/{path}");
    let mut n = 2;
    while root.join(&target).exists() {
        let (stem, extension) = path.rsplit_once('.').unwrap_or((&path, ""));
        target = if extension.is_empty() { format!(".trash/{stem} {n}") } else { format!(".trash/{stem} {n}.{extension}") };
        n += 1;
    }
    let to = root.join(&target);
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    std::fs::rename(root.join(&path), &to).map_err(|e| format!("{path}: {e}"))?;
    Ok(target)
}

/// Puts back a note from the trash, where it was.
pub fn untrash(root: &Path, trashed: &str, path: &str) -> Result<(), String> {
    let to = root.join(normalize(path).ok_or_else(|| format!("{path}: outside the notes"))?);
    // Only from the vault's own trash.
    let from = normalize(trashed).filter(|t| t.starts_with(".trash/")).ok_or_else(|| format!("{trashed}: outside the notes"))?;
    if to.exists() {
        return Err(format!("{path}: taken again"));
    }
    std::fs::rename(root.join(from), &to).map_err(|e| format!("{path}: {e}"))
}

/// A note renamed: its file gets `name` (its extension kept), in the same
/// folder, and the notes that link to it follow: `[[old]]`, `![[old.png]]`,
/// Markdown links by path, `sioul:note/` addresses. Returns its new path and
/// the notes changed.
pub fn rename(vault: &Vault, path: &str, name: &str) -> Result<(String, Vec<String>), String> {
    let note = vault.note(path).ok_or_else(|| format!("{path}: no such note"))?;
    let clean: String = name.trim().chars().filter(|c| !matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '#' | '^' | '[' | ']') && !c.is_control()).collect();
    // Nothing hidden: a name starting with a dot would take the note out of the list.
    let clean = text_stem(clean.trim().trim_start_matches('.')).trim().to_string();
    if clean.is_empty() {
        return Err(format!("{path}: a note needs a name"));
    }
    let clean = not_a_device(clean);
    let extension = path.rsplit_once('.').map_or("md", |(_, e)| e);
    let folder = note.folder();
    let file = format!("{clean}.{extension}");
    let new_path = if folder.is_empty() { file.clone() } else { format!("{folder}/{file}") };
    if new_path == path {
        return Ok((new_path, Vec::new()));
    }
    if vault.root.join(&new_path).exists() {
        return Err(format!("{new_path}: taken"));
    }
    std::fs::rename(vault.root.join(path), vault.root.join(&new_path)).map_err(|e| format!("{path}: {e}"))?;
    // The ways other notes name it, and what they say now.
    let old_name = path.rsplit('/').next().unwrap_or(path);
    let old_stem = if note.kind == NoteKind::Text { text_stem(old_name) } else { old_name };
    let new_stem = if note.kind == NoteKind::Text { clean.as_str() } else { file.as_str() };
    let mut changed = Vec::new();
    for other in vault.notes.iter().filter(|n| n.kind == NoteKind::Text && n.path != path) {
        let Ok(text) = std::fs::read_to_string(vault.root.join(&other.path)) else { continue };
        let mut next = text.clone();
        for (old, new) in [
            (format!("[[{old_stem}]]"), format!("[[{new_stem}]]")),
            (format!("[[{old_stem}|"), format!("[[{new_stem}|")),
            (format!("[[{old_stem}#"), format!("[[{new_stem}#")),
            (format!("[[{}]]", text_stem(path)), format!("[[{}]]", text_stem(&new_path))),
            (uri_of(path), uri_of(&new_path)),
            (format!("]({})", encode_path(&relative(other.folder(), path))), format!("]({})", encode_path(&relative(other.folder(), &new_path)))),
            (format!("](<{}>)", relative(other.folder(), path)), format!("](<{}>)", relative(other.folder(), &new_path))),
        ] {
            next = next.replace(&old, &new);
        }
        if next != text {
            write(&vault.root, &other.path, &next).map_err(|e| format!("{}: {e}", other.path))?;
            changed.push(other.path.clone());
        }
    }
    Ok((new_path, changed))
}

/// A folder's name made safe on every system: no slash, nothing hidden.
fn folder_name(name: &str) -> Result<String, String> {
    let clean: String = name.trim().chars().filter(|c| !matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '#' | '^' | '[' | ']')).collect();
    let clean = clean.trim().trim_start_matches('.').trim().to_string();
    if clean.is_empty() { Err(format!("{name}: a folder needs a name")) } else { Ok(not_a_device(clean)) }
}

/// A name Windows keeps for a device ("CON", "NUL", "AUX", "PRN", "COM1",
/// "LPT1"…, whatever follows its first dot) taken out of its way with a "_"
/// before that dot: "Nul.md" would write to no file there, "aux.txt.md" too
/// ("aux_.txt.md" does not), and notes travel between systems.
fn not_a_device(name: String) -> String {
    let at = name.find('.').unwrap_or(name.len());
    let base = name[..at].trim().to_ascii_lowercase();
    let numbered = base.len() == 4 && (base.starts_with("com") || base.starts_with("lpt")) && base.as_bytes()[3].is_ascii_digit();
    if numbered || matches!(base.as_str(), "con" | "prn" | "aux" | "nul") { format!("{}_{}", &name[..at], &name[at..]) } else { name }
}

/// A new folder in `parent` ("" for the top); returns its path.
pub fn make_folder(root: &Path, parent: &str, name: &str) -> Result<String, String> {
    let name = folder_name(name)?;
    let parent = normalize(parent).unwrap_or_default();
    let path = if parent.is_empty() { name } else { format!("{parent}/{name}") };
    if root.join(&path).exists() {
        return Err(format!("{path}: taken"));
    }
    std::fs::create_dir_all(root.join(&path)).map_err(|e| format!("{path}: {e}"))?;
    Ok(path)
}

/// A folder renamed in place, its notes with it; the notes that name them by
/// their path follow (`[[folder/note]]`, Markdown links, `sioul:note/`
/// addresses). Returns its new path and each note moved, from and to.
pub fn rename_folder(vault: &Vault, path: &str, name: &str) -> Result<(String, Vec<(String, String)>), String> {
    let path = normalize(path).ok_or_else(|| format!("{path}: outside the notes"))?;
    let name = folder_name(name)?;
    let new_path = match path.rsplit_once('/') {
        Some((parent, _)) => format!("{parent}/{name}"),
        None => name,
    };
    if new_path == path {
        return Ok((new_path, Vec::new()));
    }
    if vault.root.join(&new_path).exists() {
        return Err(format!("{new_path}: taken"));
    }
    std::fs::rename(vault.root.join(&path), vault.root.join(&new_path)).map_err(|e| format!("{path}: {e}"))?;
    let prefix = format!("{path}/");
    let moved: Vec<(String, String)> = vault.notes.iter().filter_map(|n| n.path.strip_prefix(&prefix).map(|rest| (n.path.clone(), format!("{new_path}/{rest}")))).collect();
    for other in vault.notes.iter().filter(|n| n.kind == NoteKind::Text) {
        let here = moved.iter().find(|(from, _)| *from == other.path).map_or(other.path.clone(), |(_, to)| to.clone());
        let Ok(text) = std::fs::read_to_string(vault.root.join(&here)) else { continue };
        let mut next = text.clone();
        for (from, to) in &moved {
            for (old, new) in [
                (format!("[[{}]]", text_stem(from)), format!("[[{}]]", text_stem(to))),
                (format!("[[{}|", text_stem(from)), format!("[[{}|", text_stem(to))),
                (uri_of(from), uri_of(to)),
                (format!("]({})", encode_path(&relative(other.folder(), from))), format!("]({})", encode_path(&relative(other.folder(), to)))),
            ] {
                next = next.replace(&old, &new);
            }
        }
        if next != text {
            write(&vault.root, &here, &next)?;
        }
    }
    Ok((new_path, moved))
}

/// An empty folder taken out; one holding anything stays, with why.
pub fn remove_folder(root: &Path, path: &str) -> Result<(), String> {
    let path = normalize(path).filter(|p| !p.is_empty()).ok_or_else(|| format!("{path}: outside the notes"))?;
    let full = root.join(&path);
    let empty = std::fs::read_dir(&full).map_err(|e| format!("{path}: {e}"))?.next().is_none();
    if !empty {
        return Err(format!("{path}: not empty"));
    }
    std::fs::remove_dir(&full).map_err(|e| format!("{path}: {e}"))
}

/// The path from a note's folder to another note: "../admin/lease.md".
fn relative(from_folder: &str, to: &str) -> String {
    let from: Vec<&str> = from_folder.split('/').filter(|p| !p.is_empty()).collect();
    let to_parts: Vec<&str> = to.split('/').collect();
    let common = from.iter().zip(&to_parts).take_while(|(a, b)| a == b).count();
    let mut parts: Vec<&str> = std::iter::repeat_n("..", from.len() - common).collect();
    parts.extend(&to_parts[common..]);
    parts.join("/")
}

/// A picture's width and height from its first bytes: PNG, GIF, JPEG, WebP.
pub fn image_size(path: &Path) -> Option<(u32, u32)> {
    use std::io::Read;
    let mut head = Vec::with_capacity(64 * 1024);
    std::fs::File::open(path).ok()?.take(256 * 1024).read_to_end(&mut head).ok()?;
    let be16 = |b: &[u8], at: usize| b.get(at..at + 2).map(|x| u32::from(u16::from_be_bytes([x[0], x[1]])));
    let le16 = |b: &[u8], at: usize| b.get(at..at + 2).map(|x| u32::from(u16::from_le_bytes([x[0], x[1]])));
    let be32 = |b: &[u8], at: usize| b.get(at..at + 4).map(|x| u32::from_be_bytes([x[0], x[1], x[2], x[3]]));
    if head.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some((be32(&head, 16)?, be32(&head, 20)?));
    }
    if head.starts_with(b"GIF8") {
        return Some((le16(&head, 6)?, le16(&head, 8)?));
    }
    if head.starts_with(b"RIFF") && head.get(8..12) == Some(b"WEBP") {
        return match head.get(12..16)? {
            b"VP8X" => {
                let three = |at: usize| head.get(at..at + 3).map(|x| u32::from(x[0]) | u32::from(x[1]) << 8 | u32::from(x[2]) << 16);
                Some((three(24)? + 1, three(27)? + 1))
            }
            b"VP8 " => Some((le16(&head, 26)? & 0x3fff, le16(&head, 28)? & 0x3fff)),
            b"VP8L" => {
                let bits = head.get(21..25).map(|x| u32::from_le_bytes([x[0], x[1], x[2], x[3]]))?;
                Some(((bits & 0x3fff) + 1, ((bits >> 14) & 0x3fff) + 1))
            }
            _ => None,
        };
    }
    if head.starts_with(&[0xff, 0xd8]) {
        // The first frame header (SOF0 to SOF15, but not DHT, JPG or DAC) holds the size.
        let mut at = 2;
        while at + 9 < head.len() {
            if head[at] != 0xff {
                at += 1;
                continue;
            }
            let marker = head[at + 1];
            if (0xc0..=0xcf).contains(&marker) && ![0xc4, 0xc8, 0xcc].contains(&marker) {
                return Some((be16(&head, at + 7)?, be16(&head, at + 5)?));
            }
            at += 2 + usize::try_from(be16(&head, at + 2)?).ok()?;
        }
    }
    None
}

/// A note's address for links.
pub fn uri_of(path: &str) -> String {
    format!("sioul:note/{}", encode_path(path))
}

/// The vault path of a note's address, when it is one.
pub fn path_of(uri: &str) -> Option<String> {
    let rest = uri.strip_prefix("sioul:note/")?;
    let path = decode(rest.split('#').next().unwrap_or(rest));
    // Addresses written before October 2026 carried each byte of an accented
    // name as a letter of its own ("SantÃ©" for "Santé"): read as meant.
    let bytes: Option<Vec<u8>> = path.chars().map(|c| u8::try_from(u32::from(c)).ok()).collect();
    Some(bytes.filter(|b| b.iter().any(|&x| x >= 0x80)).and_then(|b| String::from_utf8(b).ok()).unwrap_or(path))
}

/// Spaces and the few characters a URI cannot hold, percent-encoded; "/" and
/// the letters of every language kept as they are (an IRI, RFC 3987), so that
/// `decode` gives the same path back ("Santé/impôts.md").
pub fn encode_path(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for c in path.chars() {
        if matches!(c, ' ' | '"' | '#' | '%' | '<' | '>' | '?' | '[' | ']' | '\\' | '^' | '`' | '{' | '|' | '}') || c.is_control() {
            for byte in c.encode_utf8(&mut [0; 4]).bytes() {
                out.push_str(&format!("%{byte:02X}"));
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// A file's address for the window (a picture, a PDF): "file:///home/…", and
/// "file:///C:/Users/…" on Windows, with "/" between its parts and what an
/// address cannot hold ("#", "?", "%", spaces) percent-encoded.
pub fn file_url(path: &Path) -> String {
    let text = path.to_string_lossy();
    // Windows writes "C:\Users\…" (or "\\?\C:\…"); elsewhere "\" belongs to a name.
    let text = if cfg!(windows) { text.replace('\\', "/").trim_start_matches("//?/").to_string() } else { text.to_string() };
    match text {
        // A share of another computer: "file://server/share/…".
        share if share.starts_with("//") => format!("file:{}", encode_path(&share)),
        rooted if rooted.starts_with('/') => format!("file://{}", encode_path(&rooted)),
        drive => format!("file:///{}", encode_path(&drive)),
    }
}

/// Percent-decoding, as links in Markdown write spaces ("my%20note.md").
pub fn decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && let Some(byte) = text.get(i + 1..i + 3).and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            out.push(byte);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

/// The front matter: the lines between a first "---" and the next, read as
/// simple YAML (`key: value`, `key: [a, b]`, `key:` then `- item` lines).
/// Returns it, and the line where the text starts.
pub fn front_matter(text: &str) -> (BTreeMap<String, Vec<String>>, usize) {
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let all: Vec<&str> = text.lines().collect();
    if all.first().map(|l| l.trim_end()) != Some("---") {
        return (out, 0);
    }
    let Some(end) = all.iter().skip(1).position(|l| matches!(l.trim_end(), "---" | "...")) else { return (BTreeMap::new(), 0) };
    let mut key: Option<String> = None;
    for line in &all[1..=end] {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let (Some(item), Some(k)) = (trimmed.strip_prefix("- "), key.as_ref()) {
            out.entry(k.clone()).or_default().push(unquote(item));
            continue;
        }
        let Some((k, value)) = trimmed.split_once(':') else { continue };
        let k = k.trim().to_lowercase();
        let value = value.trim();
        key = Some(k.clone());
        let entry = out.entry(k).or_default();
        if let Some(list) = value.strip_prefix('[').and_then(|v| v.strip_suffix(']')) {
            entry.extend(list.split(',').map(unquote).filter(|v| !v.is_empty()));
        } else if !value.is_empty() {
            entry.push(unquote(value));
        }
    }
    (out, end + 2)
}

fn unquote(value: &str) -> String {
    let value = value.trim();
    let quoted = value.len() >= 2 && ((value.starts_with('"') && value.ends_with('"')) || (value.starts_with('\'') && value.ends_with('\'')));
    if quoted { value[1..value.len() - 1].to_string() } else { value.to_string() }
}

/// Front matter keys whose values are links.
const FRONT_LINKS: &[&str] = &["task", "tasks", "event", "events", "mail", "mails", "contact", "contacts", "draft", "drafts", "links", "related", "case"];

/// Reads one note's text: its title, tags, links and checkboxes. Links are
/// kept as written; `Vault` resolves those that point at notes.
pub fn read(path: &str, text: &str) -> Note {
    let (front, body_start) = front_matter(text);
    let mut note = Note { path: path.to_string(), front: front.clone(), ..Note::default() };
    note.tags.extend(front.get("tags").into_iter().flatten().chain(front.get("tag").into_iter().flatten()).map(|t| t.trim_start_matches('#').to_string()));
    note.aliases.extend(front.get("aliases").into_iter().flatten().chain(front.get("alias").into_iter().flatten()).cloned());
    for key in FRONT_LINKS {
        for value in front.get(*key).into_iter().flatten() {
            let target = if *key == "case" && !value.contains(':') { format!("sioul:case/{value}") } else { value.clone() };
            note.links.push(NoteLink { target, anchor: String::new(), text: key.to_string(), line: 1 });
        }
    }
    let mut fenced = false;
    for (n, line) in text.lines().enumerate().skip(body_start) {
        let number = n + 1;
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }
        if note.title.is_empty()
            && let Some(heading) = trimmed.strip_prefix("# ")
        {
            note.title = heading.trim().to_string();
        }
        if let Some(checkbox) = checkbox(trimmed, number) {
            note.checkboxes.push(checkbox);
        }
        let plain = without_code(line);
        links_in(&plain, number, &mut note.links);
        for tag in tags_in(&plain) {
            if !note.tags.contains(&tag) {
                note.tags.push(tag);
            }
        }
    }
    if let Some(title) = front.get("title").and_then(|t| t.first()) {
        note.title = title.clone();
    }
    if note.title.is_empty() {
        note.title = note.stem().to_string();
    }
    note
}

/// "- [ ] text", "* [x] text", "1. [ ] text".
fn checkbox(line: &str, number: usize) -> Option<Checkbox> {
    let rest = line.strip_prefix(['-', '*', '+']).map(str::trim_start).or_else(|| {
        let digits = line.chars().take_while(char::is_ascii_digit).count();
        (digits > 0).then(|| line[digits..].strip_prefix(['.', ')'])).flatten().map(str::trim_start)
    })?;
    let (mark, text) = (rest.get(..3)?, rest.get(3..)?);
    let done = match mark {
        "[ ]" => false,
        "[x]" | "[X]" => true,
        _ => return None,
    };
    let text = text.trim();
    (!text.is_empty()).then(|| Checkbox { line: number, done, text: text.to_string() })
}

/// A line with its `inline code` blanked, so links and tags inside are not taken.
fn without_code(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut in_code = false;
    for c in line.chars() {
        if c == '`' {
            in_code = !in_code;
            out.push(' ');
        } else {
            out.push(if in_code { ' ' } else { c });
        }
    }
    out
}

/// Wikilinks and Markdown links in a line.
fn links_in(line: &str, number: usize, out: &mut Vec<NoteLink>) {
    // [[target#anchor|text]], and ![[embedded]].
    let mut rest = line;
    while let Some(open) = rest.find("[[") {
        let after = &rest[open + 2..];
        let Some(close) = after.find("]]") else { break };
        let inner = &after[..close];
        let (target, text) = inner.split_once('|').map_or((inner, inner), |(t, x)| (t, x));
        let (target, anchor) = target.split_once('#').unwrap_or((target, ""));
        if !target.trim().is_empty() || !anchor.is_empty() {
            out.push(NoteLink { target: format!("wiki:{}", target.trim()), anchor: anchor.trim().to_string(), text: text.trim().to_string(), line: number });
        }
        rest = &after[close + 2..];
    }
    // [text](target "title"), [text](<target with spaces>).
    let bytes = line.as_bytes();
    let mut i = 0;
    while let Some(found) = line[i..].find("](") {
        let close_text = i + found;
        let Some(open_text) = line[..close_text].rfind('[') else {
            i = close_text + 2;
            continue;
        };
        let start = close_text + 2;
        let (target, end) = if bytes.get(start) == Some(&b'<') {
            match line[start + 1..].find('>') {
                Some(e) => (&line[start + 1..start + 1 + e], start + 1 + e + 1),
                None => break,
            }
        } else {
            let mut depth = 0;
            let mut end = None;
            for (k, c) in line[start..].char_indices() {
                match c {
                    '(' => depth += 1,
                    ')' if depth == 0 => {
                        end = Some(start + k);
                        break;
                    }
                    ')' => depth -= 1,
                    _ => {}
                }
            }
            let Some(end) = end else { break };
            (line[start..end].split(" \"").next().unwrap_or(""), end)
        };
        let target = target.trim();
        if !target.is_empty() {
            let (target, anchor) = match target.split_once('#') {
                Some((t, a)) if !t.contains(':') || t.starts_with("sioul:note/") => (t, a),
                _ => (target, ""),
            };
            out.push(NoteLink { target: target.to_string(), anchor: decode(anchor), text: line[open_text + 1..close_text].trim().to_string(), line: number });
        }
        i = end.max(start);
    }
    // Bare addresses written as they are: <mid:…>, sioul:…
    for word in line.split_whitespace() {
        let word = word.trim_matches(|c: char| matches!(c, '<' | '>' | '(' | ')' | ',' | ';' | '.'));
        if (word.starts_with("mid:") || word.starts_with("sioul:")) && !out.iter().any(|l| l.line == number && l.target == word) {
            out.push(NoteLink { target: word.to_string(), anchor: String::new(), text: String::new(), line: number });
        }
    }
}

/// `#tag` and `#nested/tag`: after a space or at the start, not all digits, not a heading.
fn tags_in(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let chars: Vec<char> = line.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if c != '#' || (i > 0 && !chars[i - 1].is_whitespace()) {
            continue;
        }
        let tag: String = chars[i + 1..].iter().take_while(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '/')).collect();
        if !tag.is_empty() && !tag.chars().all(|c| c.is_ascii_digit() || c == '-' || c == '/') && !out.contains(&tag) {
            out.push(tag);
        }
    }
    out
}

/// Every note of a vault, with what links where.
#[derive(Debug, Clone, Default)]
pub struct Vault {
    pub root: PathBuf,
    pub notes: Vec<Note>,
    /// Every folder, empty ones too: "admin", "admin/letters".
    pub folders: Vec<String>,
}

impl Vault {
    /// Reads every Markdown file under `root`, and lists the pictures, PDFs
    /// and sounds beside them; hidden folders and tools' own are skipped.
    pub fn open(root: &Path) -> Vault {
        let mut notes = Vec::new();
        let mut folders = Vec::new();
        let mut stack = vec![root.to_path_buf()];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else { continue };
            for entry in entries.filter_map(Result::ok) {
                let path = entry.path();
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with('.') || SKIPPED.contains(&name.as_str()) {
                    continue;
                }
                let Ok(kind) = entry.file_type() else { continue };
                if kind.is_dir() {
                    folders.push(path.strip_prefix(root).unwrap_or(&path).components().map(|c| c.as_os_str().to_string_lossy().to_string()).collect::<Vec<_>>().join("/"));
                    stack.push(path);
                } else if kind.is_file() && let Some(file_kind) = kind_of_file(&name) {
                    let relative = path.strip_prefix(root).unwrap_or(&path).components().map(|c| c.as_os_str().to_string_lossy().to_string()).collect::<Vec<_>>().join("/");
                    let mut note = if file_kind == NoteKind::Text {
                        let Ok(text) = std::fs::read_to_string(&path) else { continue };
                        read(&relative, &text)
                    } else {
                        let title = name.rsplit_once('.').map_or(name.as_str(), |(stem, _)| stem).to_string();
                        Note { path: relative, kind: file_kind, title, ..Note::default() }
                    };
                    note.modified = entry.metadata().ok().and_then(|m| m.modified().ok()).and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs() as i64);
                    notes.push(note);
                }
            }
        }
        notes.sort_by(|a, b| a.path.to_lowercase().cmp(&b.path.to_lowercase()));
        folders.sort_by_key(|f| f.to_lowercase());
        let mut vault = Vault { root: root.to_path_buf(), notes, folders };
        vault.resolve_all();
        vault
    }

    /// Links to notes, made into vault paths: "wiki:letters" and "../admin/letters.md" → "sioul:note/admin/letters.md".
    fn resolve_all(&mut self) {
        let resolved: Vec<Vec<Option<String>>> = self.notes.iter().map(|note| note.links.iter().map(|l| self.resolve(note.folder(), &l.target)).collect()).collect();
        for (note, targets) in self.notes.iter_mut().zip(resolved) {
            for (link, target) in note.links.iter_mut().zip(targets) {
                if let Some(path) = target {
                    link.target = uri_of(&path);
                }
            }
        }
    }

    /// The vault path a link from a note in `folder` points at, when it is a note.
    pub fn resolve(&self, folder: &str, target: &str) -> Option<String> {
        if let Some(name) = target.strip_prefix("wiki:") {
            return self.by_name(folder, name);
        }
        if let Some(path) = path_of(target) {
            return self.notes.iter().any(|n| n.path == path).then_some(path);
        }
        if target.contains(':') || target.starts_with('/') {
            return None;
        }
        let decoded = decode(target);
        let joined = normalize(&if folder.is_empty() { decoded.clone() } else { format!("{folder}/{decoded}") })?;
        let candidates = [joined.clone(), format!("{joined}.md"), format!("{joined}.txt")];
        candidates.into_iter().find(|c| self.notes.iter().any(|n| &n.path == c))
    }

    /// Obsidian's rule for `[[name]]`: a path that ends with it, the same
    /// folder first, then the shortest path; an alias otherwise.
    fn by_name(&self, folder: &str, name: &str) -> Option<String> {
        let wanted = text_stem(name.trim()).to_lowercase();
        if wanted.is_empty() {
            return None;
        }
        let matches = |n: &&Note| {
            let path = n.path.to_lowercase();
            let path = text_stem(&path);
            path == wanted || path.ends_with(&format!("/{wanted}"))
        };
        let mut found: Vec<&Note> = self.notes.iter().filter(matches).collect();
        if found.is_empty() {
            found = self.notes.iter().filter(|n| n.aliases.iter().any(|a| a.to_lowercase() == wanted)).collect();
        }
        found.sort_by_key(|n| (n.folder() != folder, n.path.matches('/').count(), n.path.len()));
        found.first().map(|n| n.path.clone())
    }

    pub fn note(&self, path: &str) -> Option<&Note> {
        self.notes.iter().find(|n| n.path == path)
    }

    /// The notes that link to this one, with the line of each link.
    pub fn backlinks(&self, path: &str) -> Vec<(&Note, &NoteLink)> {
        let uri = uri_of(path);
        self.notes.iter().filter(|n| n.path != path).flat_map(|n| n.links.iter().filter(|l| l.target == uri).map(move |l| (n, l))).collect()
    }

    /// The notes whose title, path or tags hold every word of `query`, case and accents ignored.
    pub fn search(&self, query: &str) -> Vec<&Note> {
        let folded = |text: &str| crate::text::fold(text).into_iter().collect::<String>();
        let words: Vec<String> = query.split_whitespace().map(folded).collect();
        self.notes
            .iter()
            .filter(|n| {
                let haystack = folded(&format!("{} {} {}", n.title, n.path, n.tags.join(" ")));
                words.iter().all(|w| haystack.contains(w.as_str()))
            })
            .collect()
    }

    /// A note's Markdown with its `[[wikilinks]]` made ordinary links to the
    /// notes they find (`sioul:note/…`), so they can be rendered and followed;
    /// one that finds nothing stays as typed. Code is left alone.
    pub fn with_links(&self, folder: &str, body: &str) -> String {
        let mut out = String::with_capacity(body.len());
        let mut fenced = false;
        for line in body.split_inclusive('\n') {
            let trimmed = line.trim_start();
            if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
                fenced = !fenced;
            }
            if fenced || !line.contains("[[") {
                out.push_str(line);
                continue;
            }
            let mut rest = line;
            while let Some(open) = rest.find("[[") {
                let after = &rest[open + 2..];
                let Some(close) = after.find("]]") else { break };
                let inner = &after[..close];
                let (target, shown) = inner.split_once('|').unwrap_or((inner, inner));
                let (name, anchor) = target.split_once('#').unwrap_or((target, ""));
                // An embed (![[…]]): a picture shows, sized to the page; anything else is a link.
                let embed = open > 0 && rest[..open].ends_with('!');
                let start = if embed { open - 1 } else { open };
                out.push_str(&rest[..start]);
                match self.by_name(folder, name) {
                    Some(path) if embed && self.note(&path).is_some_and(|n| n.kind == NoteKind::Image) => {
                        out.push_str(&self.picture(&path));
                    }
                    Some(path) => {
                        let anchor = if anchor.is_empty() { String::new() } else { format!("#{}", encode_path(anchor)) };
                        out.push_str(&format!("[{}](<{}{anchor}>)", shown.trim(), uri_of(&path)));
                    }
                    None => out.push_str(&rest[start..open + 2 + close + 2]),
                }
                rest = &after[close + 2..];
            }
            out.push_str(rest);
        }
        out
    }

    /// A picture of the vault as HTML, at most 560 pixels wide.
    fn picture(&self, path: &str) -> String {
        let file = self.root.join(path);
        let url = file_url(&file);
        match image_size(&file) {
            Some((width, height)) if width > 0 => {
                let shown = width.min(560);
                format!("<img src=\"{url}\" width=\"{shown}\" height=\"{}\">", u64::from(height) * u64::from(shown) / u64::from(width))
            }
            _ => format!("<img src=\"{url}\" width=\"560\">"),
        }
    }

    /// Where a note's file is.
    pub fn file(&self, path: &str) -> Option<PathBuf> {
        let normal = normalize(path)?;
        Some(self.root.join(normal))
    }
}

/// "a/./b/../c.md" → "a/c.md"; None when it leaves the vault.
pub fn normalize(path: &str) -> Option<String> {
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split(['/', '\\']) {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            // On Windows "C:" leads to another drive when joined ("sioul:note/C:/…"
            // from someone else's task), and "note.md:x" to a hidden stream.
            other if cfg!(windows) && other.contains(':') => return None,
            other => parts.push(other),
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

/// A file name for a new note from its title: the title itself, without what
/// file systems refuse.
pub fn file_name(title: &str) -> String {
    let clean: String = title.trim().chars().map(|c| if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '#' | '^' | '[' | ']') || c.is_control() { '-' } else { c }).collect();
    let clean = clean.trim_matches(['.', ' ', '-']).to_string();
    format!("{}.md", if clean.is_empty() { "Note".to_string() } else { not_a_device(clean) })
}

/// Writes a note next to its place, then moves it in.
pub fn write(root: &Path, path: &str, text: &str) -> Result<PathBuf, String> {
    let relative = normalize(path).ok_or_else(|| format!("{path}: outside the notes"))?;
    if kind_of_file(&relative) != Some(NoteKind::Text) {
        return Err(format!("{path}: not a Markdown file"));
    }
    let target = root.join(&relative);
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    let extension = target.extension().and_then(|e| e.to_str()).unwrap_or("md");
    let temporary = target.with_extension(format!("{extension}.new"));
    std::fs::write(&temporary, text).map_err(|e| format!("{}: {e}", temporary.display()))?;
    std::fs::rename(&temporary, &target).map_err(|e| format!("{}: {e}", target.display()))?;
    Ok(target)
}

/// A free path for a new note in `folder`: "Title.md", else "Title 2.md"…
pub fn free_path(root: &Path, folder: &str, title: &str) -> String {
    let name = file_name(title);
    let stem = name.trim_end_matches(".md").to_string();
    let join = |file: &str| if folder.is_empty() { file.to_string() } else { format!("{}/{file}", folder.trim_matches('/')) };
    let mut path = join(&name);
    let mut n = 2;
    while root.join(&path).exists() {
        path = join(&format!("{stem} {n}.md"));
        n += 1;
    }
    path
}

/// A new note's text: front matter with its links, a title, then the body.
pub fn new_text(title: &str, front: &[(&str, Vec<String>)], body: &str) -> String {
    let mut out = String::new();
    let filled: Vec<&(&str, Vec<String>)> = front.iter().filter(|(_, v)| !v.is_empty()).collect();
    if !filled.is_empty() {
        out.push_str("---\n");
        for (key, values) in filled {
            if values.len() == 1 {
                out.push_str(&format!("{key}: {}\n", yaml_value(&values[0])));
            } else {
                out.push_str(&format!("{key}:\n"));
                for value in values {
                    out.push_str(&format!("  - {}\n", yaml_value(value)));
                }
            }
        }
        out.push_str("---\n\n");
    }
    out.push_str(&format!("# {}\n\n", title.trim()));
    if !body.trim().is_empty() {
        out.push_str(body.trim_end());
        out.push('\n');
    }
    out
}

/// The text with `uri` among its front matter's `links`; the front matter is
/// made when there is none. Everything else stays as written.
pub fn add_front_link(text: &str, uri: &str) -> String {
    let (front, start) = front_matter(text);
    if FRONT_LINKS.iter().any(|k| front.get(*k).is_some_and(|values| values.iter().any(|v| v == uri))) {
        return text.to_string();
    }
    let item = format!("  - {}", yaml_value(uri));
    if start == 0 {
        return format!("---\nlinks:\n{item}\n---\n\n{text}");
    }
    let lines: Vec<&str> = text.lines().collect();
    let close = start - 1;
    let mut out: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
    let at = (1..close).find(|&i| !lines[i].starts_with([' ', '\t']) && lines[i].split_once(':').is_some_and(|(k, _)| k.trim().eq_ignore_ascii_case("links")));
    match at {
        // `links:` then its items: one more after the last.
        Some(i) if lines[i].split_once(':').is_some_and(|(_, v)| v.trim().is_empty()) => {
            let mut j = i + 1;
            while j < close && lines[j].trim_start().starts_with("- ") {
                j += 1;
            }
            out.insert(j, item);
        }
        // `links: a` or `links: [a, b]`: written again as a list.
        Some(i) => {
            let mut block = vec!["links:".to_string()];
            block.extend(front.get("links").into_iter().flatten().map(|v| format!("  - {}", yaml_value(v))));
            block.push(item);
            out.splice(i..=i, block);
        }
        None => out.splice(close..close, ["links:".to_string(), item]).for_each(drop),
    }
    let mut joined = out.join("\n");
    if text.ends_with('\n') {
        joined.push('\n');
    }
    joined
}

/// The text without the front matter links `same` picks, and whether there
/// were any. A link written in the text itself is left: it is the note's words.
pub fn remove_front_link(text: &str, same: impl Fn(&str) -> bool) -> (String, bool) {
    let (_, start) = front_matter(text);
    if start == 0 {
        return (text.to_string(), false);
    }
    let close = start - 1;
    let lines: Vec<&str> = text.lines().collect();
    let mut kept: Vec<String> = vec![lines[0].to_string()];
    let mut removed = false;
    let mut key = String::new();
    for line in &lines[1..close] {
        let trimmed = line.trim();
        if let Some(item) = trimmed.strip_prefix("- ") {
            if FRONT_LINKS.contains(&key.as_str()) && same(&unquote(item)) {
                removed = true;
                continue;
            }
        } else if let Some((k, value)) = trimmed.split_once(':') {
            key = k.trim().to_lowercase();
            let value = value.trim();
            if FRONT_LINKS.contains(&key.as_str()) && !value.is_empty() {
                if let Some(list) = value.strip_prefix('[').and_then(|v| v.strip_suffix(']')) {
                    let all: Vec<String> = list.split(',').map(unquote).filter(|v| !v.is_empty()).collect();
                    let left: Vec<String> = all.iter().filter(|v| !same(v)).cloned().collect();
                    if left.len() != all.len() {
                        removed = true;
                        if !left.is_empty() {
                            kept.push(format!("{key}: [{}]", left.iter().map(|v| yaml_value(v)).collect::<Vec<_>>().join(", ")));
                        }
                        continue;
                    }
                } else if same(&unquote(value)) {
                    removed = true;
                    continue;
                }
            }
        }
        kept.push(line.to_string());
    }
    if !removed {
        return (text.to_string(), false);
    }
    // A link key left without items goes too.
    let mut tidy: Vec<String> = Vec::new();
    for (i, line) in kept.iter().enumerate() {
        let lone = line.trim_end().strip_suffix(':').is_some_and(|k| FRONT_LINKS.contains(&k.trim().to_lowercase().as_str()) && !line.starts_with([' ', '\t']));
        if lone && !kept.get(i + 1).is_some_and(|next| next.trim_start().starts_with("- ")) {
            continue;
        }
        tidy.push(line.clone());
    }
    let rest: Vec<&str> = lines[close..].to_vec();
    // Nothing left in the front matter: it goes, with the empty line after it.
    let mut out: Vec<String> = if tidy.len() == 1 {
        let after: Vec<String> = rest[1..].iter().map(|l| l.to_string()).collect();
        after.into_iter().skip_while(|l| l.trim().is_empty()).collect()
    } else {
        tidy.into_iter().chain(rest.iter().map(|l| l.to_string())).collect()
    };
    if out.is_empty() {
        out.push(String::new());
    }
    let mut joined = out.join("\n");
    if text.ends_with('\n') && !joined.ends_with('\n') {
        joined.push('\n');
    }
    (joined, true)
}

/// A YAML scalar, quoted when it would not read back as written; on one
/// line, as a line break would start another key of the front matter.
fn yaml_value(value: &str) -> String {
    let value: String = value.chars().filter(|c| !c.is_control()).collect();
    let value = value.as_str();
    let plain = !value.is_empty() && !value.starts_with([' ', '-', '[', '{', '#', '&', '*', '!', '|', '>', '\'', '"', '%', '@', '`']) && !value.contains(": ") && !value.contains(" #") && !value.ends_with(':');
    if plain { value.to_string() } else { format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\"")) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folders_made_renamed_taken_out() {
        let root = std::env::temp_dir().join(format!("sioul-folders-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("admin")).unwrap();
        std::fs::write(root.join("admin/lease.md"), "# Lease\n").unwrap();
        std::fs::write(root.join("index.md"), "See [[admin/lease]] and sioul:note/admin/lease.md.\n").unwrap();
        assert_eq!(make_folder(&root, "admin", "letters").unwrap(), "admin/letters");
        assert!(make_folder(&root, "", "admin").is_err(), "taken");
        let vault = Vault::open(&root);
        assert_eq!(vault.folders, vec!["admin".to_string(), "admin/letters".into()]);
        let (new, moved) = rename_folder(&vault, "admin", "papers").unwrap();
        assert_eq!((new.as_str(), moved), ("papers", vec![("admin/lease.md".to_string(), "papers/lease.md".to_string())]));
        let index = std::fs::read_to_string(root.join("index.md")).unwrap();
        assert!(index.contains("[[papers/lease]]") && index.contains(&uri_of("papers/lease.md")), "{index}");
        assert!(remove_folder(&root, "papers").is_err(), "it holds a note");
        remove_folder(&root, "papers/letters").unwrap();
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_note_renamed_takes_its_links_along() {
        let dir = std::env::temp_dir().join(format!("sioul-rename-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("admin")).unwrap();
        std::fs::write(dir.join("admin/lease.md"), "# Lease\n").unwrap();
        std::fs::write(dir.join("plan.md"), "See [[lease]], [[lease|the lease]], [[lease#signed]] and [the file](admin/lease.md).\n").unwrap();
        std::fs::write(dir.join("admin/other.md"), "Also <sioul:note/admin/lease.md> and [here](lease.md).\n").unwrap();
        let vault = Vault::open(&dir);
        let (path, changed) = rename(&vault, "admin/lease.md", "Lease 2026").unwrap();
        assert_eq!(path, "admin/Lease 2026.md");
        assert_eq!(changed.len(), 2);
        let read = |p: &str| std::fs::read_to_string(dir.join(p)).unwrap();
        assert_eq!(read("plan.md"), "See [[Lease 2026]], [[Lease 2026|the lease]], [[Lease 2026#signed]] and [the file](admin/Lease%202026.md).\n");
        assert_eq!(read("admin/other.md"), "Also <sioul:note/admin/Lease%202026.md> and [here](Lease%202026.md).\n");
        // To the trash, out of the notes, and back.
        let trashed = trash(&dir, &path).unwrap();
        assert_eq!(trashed, ".trash/admin/Lease 2026.md");
        assert!(Vault::open(&dir).note(&path).is_none());
        assert!(untrash(&dir, "../elsewhere.md", "back.md").is_err(), "only from the vault's trash");
        untrash(&dir, &trashed, &path).unwrap();
        assert!(Vault::open(&dir).note(&path).is_some());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn pictures_pdfs_and_sounds_are_notes_too() {
        let dir = std::env::temp_dir().join(format!("sioul-media-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("memos")).unwrap();
        std::fs::write(dir.join("plan.md"), "# Plan\n\n![[scan.png]] and [[memo.ogg]]\n").unwrap();
        // A PNG's first bytes: 1200 × 300.
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        png.extend(1200u32.to_be_bytes());
        png.extend(300u32.to_be_bytes());
        png.extend([8, 2, 0, 0, 0]);
        std::fs::write(dir.join("scan.png"), &png).unwrap();
        std::fs::write(dir.join("memos/memo.ogg"), b"OggS").unwrap();
        std::fs::write(dir.join("form.pdf"), b"%PDF-1.7").unwrap();
        // Nextcloud Notes writes `.txt` by default: Markdown all the same.
        std::fs::write(dir.join("shopping.txt"), "Shopping\n\n- [ ] bread\n").unwrap();
        std::fs::write(dir.join("data.csv"), "left out").unwrap();
        let vault = Vault::open(&dir);
        let kinds: Vec<(&str, NoteKind)> = vault.notes.iter().map(|n| (n.path.as_str(), n.kind)).collect();
        assert_eq!(kinds, vec![("form.pdf", NoteKind::Pdf), ("memos/memo.ogg", NoteKind::Audio), ("plan.md", NoteKind::Text), ("scan.png", NoteKind::Image), ("shopping.txt", NoteKind::Text)]);
        assert_eq!(vault.note("shopping.txt").map(|n| n.stem()), Some("shopping"));
        assert_eq!(vault.resolve("", "wiki:shopping").as_deref(), Some("shopping.txt"));
        assert!(write(&dir, "shopping.txt", "Shopping\n\n- [x] bread\n").is_ok(), "a .txt note is written back as it is");
        assert!(write(&dir, "data.csv", "x").is_err());
        assert_eq!(vault.note("memos/memo.ogg").map(|n| n.title.as_str()), Some("memo"));
        assert_eq!(image_size(&dir.join("scan.png")), Some((1200, 300)));
        let body = vault.with_links("", "![[scan.png]] and [[memo.ogg]]");
        assert!(body.contains("width=\"560\" height=\"140\""), "{body}");
        assert!(body.contains("[memo.ogg](<sioul:note/memos/memo.ogg>)"), "{body}");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn front_links_come_and_go() {
        let bare = "# Plan\n\nText.\n";
        let with = add_front_link(bare, "sioul:task/abc");
        assert_eq!(with, "---\nlinks:\n  - sioul:task/abc\n---\n\n# Plan\n\nText.\n");
        assert_eq!(read("plan.md", &with).front["links"], vec!["sioul:task/abc"]);
        let two = add_front_link(&with, "mid:x@example.org");
        assert_eq!(read("plan.md", &two).front["links"], vec!["sioul:task/abc", "mid:x@example.org"]);
        assert_eq!(add_front_link(&two, "mid:x@example.org"), two, "a link is added once");
        let inline = "---\ntags: [a]\nlinks: [sioul:task/abc]\n---\n# T\n";
        let grown = add_front_link(inline, "sioul:event/e");
        assert_eq!(read("t.md", &grown).front["links"], vec!["sioul:task/abc", "sioul:event/e"]);
        assert!(grown.contains("tags: [a]"), "{grown}");
        let (less, removed) = remove_front_link(&two, |v| v == "sioul:task/abc");
        assert!(removed);
        assert_eq!(read("plan.md", &less).front["links"], vec!["mid:x@example.org"]);
        let (none, _) = remove_front_link(&less, |v| v == "mid:x@example.org");
        assert_eq!(none, bare, "an empty front matter goes");
        let (kept, removed) = remove_front_link("---\ntags: [a]\nevent: sioul:event/e\n---\n# T\n", |v| v == "sioul:event/e");
        assert!(removed);
        assert_eq!(kept, "---\ntags: [a]\n---\n# T\n");
    }

    const LETTERS: &str = "---\ntitle: Outbox\ntags: [admin, letters]\naliases:\n  - Letters\ntask: sioul:task/form\n---\n\n# Every letter\n\n\
        See [the plan](../plan/plan.md#october) and [[Overview]], or [[plan#Money|money]].\n\
        From <mid:abc@caf.example>, about #housing and #health/care, not #1453 nor `#code`.\n\
        - [ ] Call the CAF\n- [x] Print the form\n```\n[[not a link]]\n```\n";

    fn vault() -> Vault {
        let mut notes = vec![
            read("admin/letters.md", LETTERS),
            read("plan/plan.md", "# The plan\n\nBack to [[Letters]].\n"),
            read("Overview.md", "# Overview\n"),
            read("old/Overview.md", "# Old overview\n"),
        ];
        notes.sort_by(|a, b| a.path.cmp(&b.path));
        let mut vault = Vault { root: PathBuf::from("/vault"), notes, folders: Vec::new() };
        vault.resolve_all();
        vault
    }

    #[test]
    fn reads_a_note() {
        let note = read("admin/letters.md", LETTERS);
        assert_eq!(note.title, "Outbox");
        assert_eq!(note.tags, vec!["admin", "letters", "housing", "health/care"]);
        assert_eq!(note.aliases, vec!["Letters"]);
        assert_eq!(note.checkboxes, vec![Checkbox { line: 13, done: false, text: "Call the CAF".into() }, Checkbox { line: 14, done: true, text: "Print the form".into() }]);
        let targets: Vec<&str> = note.links.iter().map(|l| l.target.as_str()).collect();
        assert_eq!(targets, vec!["sioul:task/form", "wiki:Overview", "wiki:plan", "../plan/plan.md", "mid:abc@caf.example"]);
        assert_eq!(note.links[3].anchor, "october");
        assert_eq!((note.links[2].anchor.as_str(), note.links[2].text.as_str()), ("Money", "money"));
    }

    #[test]
    fn links_resolve_and_come_back() {
        let vault = vault();
        let letters = vault.note("admin/letters.md").unwrap();
        let targets: Vec<&str> = letters.links.iter().map(|l| l.target.as_str()).collect();
        // [[Overview]]: the shortest path wins over old/Overview.md.
        assert_eq!(targets, vec!["sioul:task/form", "sioul:note/Overview.md", "sioul:note/plan/plan.md", "sioul:note/plan/plan.md", "mid:abc@caf.example"]);
        // [[Letters]] finds the note by its alias.
        let back: Vec<&str> = vault.backlinks("admin/letters.md").iter().map(|(n, _)| n.path.as_str()).collect();
        assert_eq!(back, vec!["plan/plan.md"]);
        assert_eq!(vault.backlinks("plan/plan.md").len(), 2);
        assert_eq!(vault.search("outbox ADMIN").len(), 1);
        assert_eq!(vault.with_links("admin", "See [[plan#Money|money]] and [[nowhere]].\n"), "See [money](<sioul:note/plan/plan.md#Money>) and [[nowhere]].\n");
        assert_eq!(normalize("a/../../b"), None);
    }

    #[test]
    fn new_notes() {
        let text = new_text("Meeting: CAF", &[("event", vec!["sioul:event/abc".into()]), ("tags", vec![])], "Notes here.");
        assert_eq!(text, "---\nevent: sioul:event/abc\n---\n\n# Meeting: CAF\n\nNotes here.\n");
        let note = read("Meeting- CAF.md", &text);
        assert_eq!((note.title.as_str(), note.links[0].target.as_str()), ("Meeting: CAF", "sioul:event/abc"));
        assert_eq!(file_name("Meeting: CAF / 5 Oct"), "Meeting- CAF - 5 Oct.md");
        assert_eq!(path_of(&uri_of("admin/my letters.md")).as_deref(), Some("admin/my letters.md"));
        // Names Windows keeps for its devices: "Nul.md" would be written nowhere there.
        assert_eq!((file_name("Nul"), file_name("com1"), file_name("Console")), ("Nul_.md".to_string(), "com1_.md".to_string(), "Console.md".to_string()));
        assert_eq!(file_name("aux.txt"), "aux_.txt.md", "the name before its first dot is what Windows reads");
        #[cfg(windows)]
        assert_eq!(normalize("C:/Windows/win.ini"), None, "another drive is outside the notes");
        // A line break in a link stays out of the front matter.
        assert_eq!(add_front_link("# T\n", "sioul:task/a\ntitle: x"), "---\nlinks:\n  - \"sioul:task/atitle: x\"\n---\n\n# T\n");
        // Accented names keep their address and come back as they were; so do addresses written before ("SantÃ©").
        assert_eq!(uri_of("Santé/impôts 2025#1.md"), "sioul:note/Santé/impôts%202025%231.md");
        assert_eq!(path_of(&uri_of("Santé/impôts 2025#1.md")).as_deref(), Some("Santé/impôts 2025#1.md"));
        assert_eq!(path_of("sioul:note/Sant\u{c3}\u{a9}.md").as_deref(), Some("Santé.md"));
        // Files as addresses for the window, on every system.
        #[cfg(unix)]
        assert_eq!(file_url(Path::new("/tmp/a b/x#1%.png")), "file:///tmp/a%20b/x%231%25.png");
        #[cfg(windows)]
        assert_eq!(file_url(Path::new(r"C:\Users\a b\x#1.png")), "file:///C:/Users/a%20b/x%231.png");
    }
}

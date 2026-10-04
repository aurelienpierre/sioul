// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Mail folders: what each is for, its name as you read it, and where it is
//! kept on disk.
//!
//! Servers say what a folder is for with special-use attributes (RFC 6154:
//! `\Sent`, `\Drafts`, `\Junk`, `\Trash`, `\Archive`, `\All`); when they do
//! not, the usual names tell it. Names travel in modified UTF-7 (RFC 3501
//! §5.1.3). Each folder is a Maildir++ subfolder of the account's Maildir, the
//! inbox being the Maildir itself, as Dovecot lays them out: "INBOX.Sent" and
//! "Sent" both become `.Sent`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Inbox,
    Drafts,
    Sent,
    Archive,
    Junk,
    Trash,
    /// Every message at once (Gmail's "All Mail"): where archiving moves mail there.
    All,
    Other,
}

impl Role {
    /// The message naming it ("folder-sent").
    pub fn message_id(self) -> &'static str {
        match self {
            Role::Inbox => "folder-inbox",
            Role::Drafts => "folder-drafts",
            Role::Sent => "folder-sent",
            Role::Archive => "folder-archive",
            Role::Junk => "folder-junk",
            Role::Trash => "folder-trash",
            Role::All => "folder-all",
            Role::Other => "folder-other",
        }
    }

    /// Its icon, from the desktop's theme.
    pub fn icon(self) -> &'static str {
        match self {
            Role::Inbox => "mail-folder-inbox",
            Role::Drafts => "document-edit",
            Role::Sent => "mail-folder-sent",
            Role::Archive => "archive-insert",
            Role::Junk => "mail-mark-junk",
            Role::Trash => "user-trash",
            Role::All => "folder-mail",
            Role::Other => "folder",
        }
    }
}

/// One folder of an account.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Folder {
    /// The name the server uses, for commands.
    pub name: String,
    /// The name as you read it.
    pub display: String,
    pub role: Role,
    /// Its Maildir++ directory under the account's Maildir: "" for the inbox, ".Sent".
    pub local: String,
    /// The server said what it is for (RFC 6154), rather than its name.
    #[serde(default)]
    pub special: bool,
}

/// The folder for a role when several have it ("Junk" and "spam" on cPanel
/// hosts): the one the server marks, else the first.
pub fn preferred(folders: &[Folder], role: Role) -> Option<&Folder> {
    folders.iter().find(|f| f.role == role && f.special).or_else(|| folders.iter().find(|f| f.role == role))
}

/// Names providers give their folders, lowercase, when they do not say what for.
const ROLE_NAMES: &[(Role, &[&str])] = &[
    (Role::Sent, &["sent", "sent items", "sent messages", "sent mail", "envoyés", "envoyes", "éléments envoyés", "messages envoyés"]),
    (Role::Drafts, &["drafts", "draft", "brouillons", "brouillon"]),
    (Role::Junk, &["junk", "spam", "junk e-mail", "junk email", "bulk mail", "courrier indésirable", "indésirables", "pourriels"]),
    (Role::Trash, &["trash", "deleted items", "deleted messages", "bin", "corbeille", "éléments supprimés"]),
    (Role::Archive, &["archive", "archives"]),
];

/// What a folder is for: the server's word first, else its name.
pub fn role_of(name: &str, special: Option<Role>) -> Role {
    if name.eq_ignore_ascii_case("INBOX") {
        return Role::Inbox;
    }
    if let Some(role) = special {
        return role;
    }
    let display = decode_utf7(name).to_lowercase();
    // The last level of the name: "INBOX.Sent", "[Gmail]/Sent Mail".
    let leaf = display.rsplit(['.', '/']).next().unwrap_or(&display).trim();
    ROLE_NAMES.iter().find(|(_, names)| names.contains(&leaf)).map_or(Role::Other, |(role, _)| *role)
}

/// A folder from what the server lists.
pub fn folder(name: &str, delimiter: Option<&str>, special: Option<Role>) -> Folder {
    let role = role_of(name, special);
    let display = display_name(name, delimiter);
    let local = if role == Role::Inbox { String::new() } else { local_dir(&display, delimiter) };
    Folder { name: name.to_string(), display, role, local, special: special.is_some_and(|s| s == role) }
}

/// "INBOX.Sent" → "Sent", "[Gmail]/Sent Mail" → "[Gmail]/Sent Mail", "Re&AOc-us" → "Reçus".
fn display_name(name: &str, delimiter: Option<&str>) -> String {
    let decoded = decode_utf7(name);
    match delimiter {
        Some(d) if !d.is_empty() => {
            let prefix = format!("INBOX{d}");
            match decoded.get(..prefix.len()) {
                Some(head) if head.eq_ignore_ascii_case(&prefix) => decoded[prefix.len()..].to_string(),
                _ => decoded,
            }
        }
        _ => decoded,
    }
}

/// The Maildir++ directory: a dot, then the levels joined by dots. Characters
/// that a level's name cannot hold on disk, on any system, are written `%XX`.
/// An empty level ("INBOX.", "a//b", "/" where "/" separates levels) is
/// written `%00`: joined as it is, it could make "." or "..", the account's
/// Maildir or the folder of every account's, which removing this folder's
/// copy would delete.
fn local_dir(display: &str, delimiter: Option<&str>) -> String {
    let levels: Vec<&str> = match delimiter {
        Some(d) if !d.is_empty() => display.split(d).collect(),
        _ => vec![display],
    };
    let encoded: Vec<String> = levels
        .iter()
        .map(|level| {
            if level.is_empty() {
                return "%00".to_string();
            }
            level
                .chars()
                .map(|c| match c {
                    '.' | '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '%' => format!("%{:02X}", c as u32),
                    c if c.is_control() => format!("%{:02X}", c as u32),
                    c => c.to_string(),
                })
                .collect()
        })
        .collect();
    format!(".{}", encoded.join("."))
}

/// Modified UTF-7 (RFC 3501 §5.1.3): `&` opens base64 of UTF-16BE, with `,` for
/// `/`, until `-`; `&-` is `&`. Anything malformed is kept as it came.
pub fn decode_utf7(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut rest = name;
    while let Some(start) = rest.find('&') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let Some(end) = after.find('-') else {
            out.push_str(&rest[start..]);
            return out;
        };
        let encoded = &after[..end];
        if encoded.is_empty() {
            out.push('&');
        } else {
            match decode_base64_utf16(encoded) {
                Some(text) => out.push_str(&text),
                None => out.push_str(&rest[start..start + 2 + end]),
            }
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

/// Encodes a name for the server, the reverse of `decode_utf7`.
pub fn encode_utf7(name: &str) -> String {
    let mut out = String::new();
    let mut pending: Vec<u16> = Vec::new();
    let flush = |pending: &mut Vec<u16>, out: &mut String| {
        if pending.is_empty() {
            return;
        }
        let bytes: Vec<u8> = pending.iter().flat_map(|u| u.to_be_bytes()).collect();
        out.push('&');
        out.push_str(&encode_base64(&bytes).replace('/', ","));
        out.push('-');
        pending.clear();
    };
    for c in name.chars() {
        if (' '..='~').contains(&c) {
            flush(&mut pending, &mut out);
            if c == '&' { out.push_str("&-") } else { out.push(c) }
        } else {
            let mut units = [0u16; 2];
            pending.extend_from_slice(c.encode_utf16(&mut units));
        }
    }
    flush(&mut pending, &mut out);
    out
}

const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn decode_base64_utf16(encoded: &str) -> Option<String> {
    let mut bits = 0u32;
    let mut count = 0;
    let mut bytes = Vec::new();
    for c in encoded.bytes() {
        let c = if c == b',' { b'/' } else { c };
        let value = BASE64.iter().position(|b| *b == c)? as u32;
        bits = (bits << 6) | value;
        count += 6;
        if count >= 8 {
            count -= 8;
            bytes.push((bits >> count) as u8);
            bits &= (1 << count) - 1;
        }
    }
    let units: Vec<u16> = bytes.chunks_exact(2).map(|p| u16::from_be_bytes([p[0], p[1]])).collect();
    String::from_utf16(&units).ok()
}

fn encode_base64(bytes: &[u8]) -> String {
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let n = chunk.iter().fold(0u32, |n, b| (n << 8) | u32::from(*b)) << (8 * (3 - chunk.len()));
        let chars = chunk.len() + 1;
        for i in 0..chars {
            out.push(BASE64[((n >> (18 - 6 * i)) & 63) as usize] as char);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_marked_folder_wins_its_role() {
        let listed = vec![folder("Junk", Some("."), Some(Role::Junk)), folder("spam", Some("."), None), folder("Archives", Some("."), None)];
        assert_eq!(listed[1].role, Role::Junk);
        assert!(listed[0].special && !listed[1].special);
        assert_eq!(preferred(&listed, Role::Junk).map(|f| f.name.as_str()), Some("Junk"));
        assert_eq!(preferred(&listed, Role::Archive).map(|f| f.name.as_str()), Some("Archives"));
        assert_eq!(preferred(&listed, Role::Trash), None);
    }

    #[test]
    fn names_in_modified_utf7() {
        assert_eq!(decode_utf7("Re&AOc-us"), "Reçus");
        assert_eq!(decode_utf7("&AMk-l&AOk-ments envoy&AOk-s"), "Éléments envoyés");
        assert_eq!(decode_utf7("Tom &- Jerry"), "Tom & Jerry");
        assert_eq!(decode_utf7("&ZeVnLIqe-"), "日本語");
        for name in ["Reçus", "Éléments envoyés", "Tom & Jerry", "日本語", "Plain"] {
            assert_eq!(decode_utf7(&encode_utf7(name)), name);
        }
        assert_eq!(encode_utf7("Reçus"), "Re&AOc-us");
    }

    #[test]
    fn roles_and_places() {
        let sent = folder("INBOX.Sent", Some("."), None);
        assert_eq!((sent.role, sent.display.as_str(), sent.local.as_str()), (Role::Sent, "Sent", ".Sent"));
        let gmail = folder("[Gmail]/Sent Mail", Some("/"), Some(Role::Sent));
        assert_eq!((gmail.role, gmail.local.as_str()), (Role::Sent, ".[Gmail].Sent Mail"));
        assert_eq!(folder("INBOX", Some("."), None).local, "");
        assert_eq!(folder("Corbeille", Some("/"), None).role, Role::Trash);
        assert_eq!(folder("INBOX.spam", Some("."), None).role, Role::Junk);
        assert_eq!(folder("Projets/2026", Some("/"), None).local, ".Projets.2026");
        assert_eq!(folder("v1.2: notes", None, None).local, ".v1%2E2%3A notes");
        assert_eq!(folder("Clients", Some("/"), None).role, Role::Other);
        // Never the Maildir itself or its parent, never an empty level.
        assert_eq!(folder("/", Some("/"), None).local, ".%00.%00");
        assert_eq!(folder("INBOX.", Some("."), None).local, ".%00");
        assert_eq!(folder("INBOX..", Some("."), None).local, ".%00.%00");
        assert_eq!(folder("a//b", Some("/"), None).local, ".a.%00.b");
        assert_eq!(folder("", None, None).local, ".%00");
    }
}

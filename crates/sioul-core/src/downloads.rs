// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! A file a site offers (docs/sites.md, "Downloads"): nothing is saved before
//! you say where, in one card over the site; then it goes to the place
//! chosen. Here are the rules the window follows, apart from Qt so that they
//! are tested alone:
//!
//! - **The places** (`Place`): the system's Downloads folder, a folder chosen
//!   in the system's "Save as" window, Notes, Papers, Letters.
//! - **The name a site gives is never a path** (`clean_name`): one plain file
//!   name, no separator, no "..", no leading dot, no character a file system
//!   refuses, nothing that turns the text around, and a sensible length, its
//!   extension kept.
//! - **A program** (`is_program`): by its extension, or by the type the site
//!   says; the card says a word of caution, and nothing opens it.
//! - **The host it comes from** (`host_of`), from its address.
//! - **The place chosen last for each site** (`Memory`), offered first the next
//!   time: this device's own (a folder is this computer's), in Sioul's state folder.
//! - **A name already there gets a number** (`numbered`), in the place's own way.
//!
//! The window asks and places (crates/sioul-app/src/downloads.rs); the file
//! is placed whole under a free name by `filelock::place_new`.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Where a file a site offers goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    /// The system's Downloads folder (`config::downloads_dir`).
    Downloads,
    /// A folder chosen in the system's "Save as" window, with the name.
    Folder,
    /// The notes folder, where new notes go: shown in Notes, tied like any note.
    Notes,
    /// The papers' folder, then the paper's form with the file attached.
    Papers,
    /// Where scans arrive, read as a letter received.
    Letters,
}

impl Place {
    /// Every place, in the card's order.
    pub const ALL: [Place; 5] = [Place::Downloads, Place::Folder, Place::Notes, Place::Papers, Place::Letters];

    /// Its id, as the window and the memory name it.
    pub fn id(self) -> &'static str {
        match self {
            Place::Downloads => "downloads",
            Place::Folder => "folder",
            Place::Notes => "notes",
            Place::Papers => "papers",
            Place::Letters => "letters",
        }
    }

    /// The place an id names; none for any other word.
    pub fn read(id: &str) -> Option<Place> {
        Place::ALL.into_iter().find(|p| p.id() == id)
    }

    /// Whether the file then lives in Sioul (Notes, Papers, Letters): its
    /// notice opens it there, and nothing half-written may stay there.
    pub fn in_sioul(self) -> bool {
        matches!(self, Place::Notes | Place::Papers | Place::Letters)
    }

    /// How a name already there gets its number in this place: as the
    /// system's folders do (`Paren`), as the papers' folder does
    /// (`Wallet::keep_file`: `Dash`), as new notes do (`notes::free_path`: `Space`).
    pub fn numbering(self) -> Numbering {
        match self {
            Place::Downloads | Place::Folder | Place::Letters => Numbering::Paren,
            Place::Papers => Numbering::Dash,
            Place::Notes => Numbering::Space,
        }
    }
}

/// How a name already taken gets its number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Numbering {
    /// "statement (2).pdf"
    Paren,
    /// "statement-2.pdf"
    Dash,
    /// "statement 2.pdf"
    Space,
}

/// The longest name kept, in characters, its extension included.
pub const NAME_CHARS: usize = 120;
/// The longest name kept, in bytes: some file systems take 255, an
/// encrypted home about 143, and a sync program adds its own marks.
pub const NAME_BYTES: usize = 200;
/// The longest extension kept as one; a longer "extension" is part of the name.
const EXTENSION_CHARS: usize = 16;
/// The name of a file a site gave no usable name for.
pub const NAMELESS: &str = "download";

/// A name made one plain file name, whatever a site sent (docs/sites.md,
/// "Downloads"): separators ("/", "\") and the characters a file system
/// refuses (`< > : " | ? *`, on every system, since a file may travel to
/// another) become "_"; control characters and those that turn text around
/// or hide in it (right-to-left marks, zero-width joiners) go; runs of dots
/// become one; dots and spaces at either end go (no hidden file, no name a
/// Windows folder drops a part of); a device's name (CON, NUL, COM1…) gets
/// "_" in front, once it is cut to `NAME_CHARS` characters and
/// `NAME_BYTES` bytes, its extension kept (so that a long name cut down to a
/// device's is caught too). Runs of spaces become one, so that a long gap cannot push a
/// program's extension out of a line ("statement.pdf      .exe"). A name
/// that is only an extension (".pdf") is "download.pdf"; nothing left: "download".
pub fn clean_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for c in name.chars() {
        if c.is_control() || hidden_mark(c) {
            continue;
        }
        let c = if matches!(c, '/' | '\\' | '<' | '>' | ':' | '"' | '|' | '?' | '*') { '_' } else if c.is_whitespace() { ' ' } else { c };
        // Runs of dots: one, so that ".." is never in a name; runs of spaces: one.
        if (c == '.' && out.ends_with('.')) || (c == ' ' && out.ends_with(' ')) {
            continue;
        }
        out.push(c);
    }
    let trimmed = out.trim_matches(|c: char| c == '.' || c.is_whitespace()).to_string();
    // Only a known type's extension (".pdf"): a name for it, its type kept.
    let only_extension = out.trim_start().starts_with('.') && !trimmed.contains(['.', ' ']) && (TYPE_EXTENSIONS.iter().any(|(_, e)| e.eq_ignore_ascii_case(&trimmed)) || crate::notes::kind_of_file(&format!("{NAMELESS}.{trimmed}")).is_some());
    let named = if trimmed.is_empty() {
        NAMELESS.to_string()
    } else if only_extension {
        format!("{NAMELESS}.{trimmed}")
    } else {
        trimmed
    };
    // A device's name with its "_" may be one character longer: cut again (the "_" stays first).
    shortened_name(&not_a_device(shortened_name(&named)))
}

/// Characters that change how a name reads without showing: the marks that
/// turn text right to left (a program "evil", U+202E, "fdp.exe" reads as
/// "evilexe.pdf"), those that join or separate without width, the byte order
/// mark, the line and paragraph separators (a one-line label breaks on
/// them), the soft hyphen, and the fillers and blanks drawn as nothing
/// (Mongolian's vowel separator, Hangul's fillers, Braille's blank, the tags).
fn hidden_mark(c: char) -> bool {
    matches!(c, '\u{00AD}' | '\u{061C}' | '\u{115F}' | '\u{1160}' | '\u{180E}' | '\u{200B}'..='\u{200F}' | '\u{2028}'..='\u{202E}' | '\u{2060}'..='\u{2064}' | '\u{2066}'..='\u{2069}' | '\u{2800}' | '\u{3164}' | '\u{FEFF}' | '\u{FFA0}' | '\u{FFF9}'..='\u{FFFB}' | '\u{E0000}'..='\u{E007F}')
}

/// A name Windows reads as a device (CON, PRN, AUX, NUL, COM0–9 and
/// LPT0–9, their superscript ¹ ² ³ too), with or without an extension, given
/// "_" in front.
fn not_a_device(name: String) -> String {
    let stem = name.split('.').next().unwrap_or("").trim_end().to_uppercase();
    let numbered = stem.chars().count() == 4 && (stem.starts_with("COM") || stem.starts_with("LPT")) && stem.chars().nth(3).is_some_and(|c| c.is_ascii_digit() || matches!(c, '¹' | '²' | '³'));
    if numbered || matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL") { format!("_{name}") } else { name }
}

/// The extension Sioul gives a name that has none, from the type the site
/// says, for the types Sioul shows (Notes, Papers, Letters): without one, a
/// file is not listed in Notes, Letters refuse it, and a desktop opens it by
/// what it finds inside.
const TYPE_EXTENSIONS: &[(&str, &str)] = &[
    ("application/pdf", "pdf"),
    ("image/png", "png"),
    ("image/jpeg", "jpg"),
    ("image/gif", "gif"),
    ("image/webp", "webp"),
    ("image/svg+xml", "svg"),
    ("image/bmp", "bmp"),
    ("image/tiff", "tif"),
    ("audio/mpeg", "mp3"),
    ("audio/ogg", "ogg"),
    ("audio/opus", "opus"),
    ("audio/wav", "wav"),
    ("audio/x-wav", "wav"),
    ("audio/flac", "flac"),
    ("audio/mp4", "m4a"),
    ("audio/aac", "aac"),
    ("text/plain", "txt"),
    ("text/markdown", "md"),
    ("application/vnd.oasis.opendocument.text", "odt"),
    ("application/vnd.oasis.opendocument.spreadsheet", "ods"),
    ("application/vnd.oasis.opendocument.presentation", "odp"),
    ("application/vnd.oasis.opendocument.graphics", "odg"),
    ("application/msword", "doc"),
    ("application/vnd.openxmlformats-officedocument.wordprocessingml.document", "docx"),
    ("application/vnd.ms-excel", "xls"),
    ("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet", "xlsx"),
    ("application/vnd.ms-powerpoint", "ppt"),
    ("application/vnd.openxmlformats-officedocument.presentationml.presentation", "pptx"),
    ("application/rtf", "rtf"),
    ("text/rtf", "rtf"),
    ("application/epub+zip", "epub"),
];

/// A site's name made one plain file name (`clean_name`), with an extension
/// from the type it says when the name has none (`TYPE_EXTENSIONS`):
/// "statement" sent as a PDF is "statement.pdf".
pub fn file_name(name: &str, mime: &str) -> String {
    let name = clean_name(name);
    if has_extension(&name) {
        return name;
    }
    let mime = mime.split(';').next().unwrap_or("").trim().to_ascii_lowercase();
    match TYPE_EXTENSIONS.iter().find(|(kind, _)| *kind == mime) {
        Some((_, extension)) => clean_name(&format!("{name}.{extension}")),
        None => name,
    }
}

/// Whether a name ends with an extension ("statement.pdf"); a desktop opens a
/// file without one by what it finds inside, so its notice offers no Open.
pub fn has_extension(name: &str) -> bool {
    !split_extension(name).1.is_empty()
}

/// Whether a name is one Sioul reads as its own: a list (`sioul-papers.toml`
/// and any other `.toml`), a list's copy kept by a rename, a move's mark.
/// Filed in Notes or Papers, Sioul could take a site's file for its own list.
pub fn is_sioul_own(name: &str) -> bool {
    let lower = name.trim().to_lowercase();
    lower.ends_with(".toml") || lower.ends_with(".before-rename") || lower.ends_with(".sioul-move")
}

/// A name and its extension ("statement", ".pdf"); "" for a name without
/// one, or whose last part is too long or not letters and digits to be one.
fn split_extension(name: &str) -> (&str, &str) {
    match name.rfind('.') {
        Some(at) if at > 0 => {
            let extension = &name[at..];
            let chars = extension.chars().count() - 1;
            if chars >= 1 && chars <= EXTENSION_CHARS && extension[1..].chars().all(char::is_alphanumeric) { (&name[..at], extension) } else { (name, "") }
        }
        _ => (name, ""),
    }
}

/// A name cut to `NAME_CHARS` characters and `NAME_BYTES` bytes, its stem
/// shortened and its extension kept.
fn shortened_name(name: &str) -> String {
    if name.chars().count() <= NAME_CHARS && name.len() <= NAME_BYTES {
        return name.to_string();
    }
    let (stem, extension) = split_extension(name);
    let mut kept = String::new();
    for c in stem.chars() {
        if kept.chars().count() + 1 + extension.chars().count() > NAME_CHARS || kept.len() + c.len_utf8() + extension.len() > NAME_BYTES {
            break;
        }
        kept.push(c);
    }
    let kept = kept.trim_end_matches(|c: char| c == '.' || c.is_whitespace());
    let kept = if kept.is_empty() { NAMELESS } else { kept };
    format!("{kept}{extension}")
}

/// The `n`th name for a file: `name` itself first, then with its number in
/// the place's own way ("statement (2).pdf", "statement-2.pdf", "statement 2.pdf").
pub fn numbered(name: &str, n: u32, numbering: Numbering) -> String {
    if n <= 1 {
        return name.to_string();
    }
    let (stem, extension) = split_extension(name);
    match numbering {
        Numbering::Paren => format!("{stem} ({n}){extension}"),
        Numbering::Dash => format!("{stem}-{n}{extension}"),
        Numbering::Space => format!("{stem} {n}{extension}"),
    }
}

/// Types a site may say a program has, whatever its name: Windows' programs
/// and installers, Linux's, macOS's disk images and packages, Java's, scripts, Android's apps.
const PROGRAM_TYPES: &[&str] = &[
    "application/x-msdownload",
    "application/x-ms-dos-executable",
    "application/x-dosexec",
    "application/x-msi",
    "application/x-ms-installer",
    "application/x-ms-shortcut",
    "application/vnd.microsoft.portable-executable",
    "application/x-executable",
    "application/x-elf",
    "application/x-sharedlib",
    "application/x-pie-executable",
    "application/x-mach-binary",
    "application/x-sh",
    "application/x-shellscript",
    "application/x-csh",
    "application/x-bat",
    "application/x-powershell",
    "application/x-apple-diskimage",
    "application/x-newton-compatible-pkg",
    "application/vnd.apple.installer+xml",
    "application/java-archive",
    "application/x-java-archive",
    "application/x-java-jnlp-file",
    "application/vnd.debian.binary-package",
    "application/x-debian-package",
    "application/x-rpm",
    "application/x-redhat-package-manager",
    "application/x-appimage",
    "application/vnd.flatpak.ref",
    "application/vnd.android.package-archive",
    "application/x-desktop",
    "application/hta",
    "text/x-sh",
    "text/x-shellscript",
    "text/javascript",
    "application/javascript",
    "application/x-javascript",
    "text/vbscript",
    "text/x-python",
    "application/x-python-code",
];

/// Whether a file is a program, a script or an installer: by its name's
/// extension (`links::is_program`, Android's installers too), or by the
/// type the site says (`PROGRAM_TYPES`). The card then says a word of
/// caution, and Sioul never opens it: shown in its folder only.
pub fn is_program(name: &str, mime: &str) -> bool {
    let mime = mime.split(';').next().unwrap_or("").trim().to_ascii_lowercase();
    crate::links::is_program(name) || crate::links::is_phone_program(name) || PROGRAM_TYPES.contains(&mime.as_str())
}

/// The host a file comes from, by its address: "secure.bank.example" for
/// `https://secure.bank.example/statements/1`, and for a page's own
/// `blob:https://secure.bank.example/…`; "" for an address that names no
/// host (a "data:" address, a file of this computer). Its login and its port left out.
pub fn host_of(url: &str) -> String {
    let url = url.trim();
    let url = url.strip_prefix("blob:").unwrap_or(url);
    let Some((scheme, rest)) = url.split_once("://") else { return String::new() };
    if !matches!(scheme.to_ascii_lowercase().as_str(), "http" | "https" | "ftp") {
        return String::new();
    }
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host = authority.rsplit_once('@').map_or(authority, |(_, host)| host);
    let host = if host.starts_with('[') { host.split_inclusive(']').next().unwrap_or(host) } else { host.split(':').next().unwrap_or(host) };
    host.trim_end_matches('.').to_ascii_lowercase()
}

/// Words from a site, cut for a line: at most `chars` characters, the middle
/// left out ("Relevé_de_comp…_2026-10.pdf"), so that the start and the
/// extension stay in view.
pub fn shortened(text: &str, chars: usize) -> String {
    let count = text.chars().count();
    if count <= chars || chars < 3 {
        return text.to_string();
    }
    let head = (chars - 1) / 2 + (chars - 1) % 2;
    let tail = chars - 1 - head;
    let start: String = text.chars().take(head).collect();
    let end: String = text.chars().skip(count - tail).collect();
    format!("{start}…{end}")
}

/// The place chosen last for one site, and the folder when it was one chosen.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Chosen {
    pub place: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub folder: String,
}

/// The place chosen last for each site, offered first the next time
/// (docs/sites.md, "Downloads"): `downloads.toml` in Sioul's state folder,
/// this device's own, since a folder is this computer's. Keyed by the site's
/// id in Sioul, else by "host:" and the host it came from.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Memory {
    #[serde(default)]
    pub site: BTreeMap<String, Chosen>,
}

/// Where the memory is kept.
pub fn memory_path() -> PathBuf {
    crate::config::state_dir().join("downloads.toml")
}

/// The memory's key for a site: its id, else its host.
pub fn site_key(site: &str, host: &str) -> String {
    if site.trim().is_empty() { format!("host:{}", host.trim()) } else { site.trim().to_string() }
}

impl Memory {
    /// Read from `path`; empty when it is not there or does not read.
    pub fn load(path: &Path) -> Memory {
        std::fs::read_to_string(path).ok().and_then(|text| toml::from_str(&text).ok()).unwrap_or_default()
    }

    /// Written whole beside `path`, then renamed over it.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        let text = toml::to_string(self).map_err(|e| e.to_string())?;
        crate::filelock::replace(path, format!("# The place a file from each site went last (docs/sites.md, \"Downloads\").\n\n{text}").as_bytes()).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// The place to offer first for a site: the one chosen last, else Downloads.
    pub fn first(&self, key: &str) -> (Place, String) {
        self.site.get(key).and_then(|c| Place::read(&c.place).map(|p| (p, c.folder.clone()))).unwrap_or((Place::Downloads, String::new()))
    }

    /// A place chosen for a site, kept; the folder only for one chosen in the
    /// system's window, the one kept before otherwise.
    pub fn chose(&mut self, key: &str, place: Place, folder: &str) {
        let kept = self.site.get(key).map(|c| c.folder.clone()).unwrap_or_default();
        let folder = if place == Place::Folder && !folder.trim().is_empty() { folder.trim().to_string() } else { kept };
        self.site.insert(key.to_string(), Chosen { place: place.id().to_string(), folder });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The name a site gives is never a path: no separator, no "..", no
    /// leading dot, nothing a file system refuses or that hides how it reads,
    /// a sensible length with its extension kept.
    #[test]
    fn a_site_s_name_is_one_plain_file_name() {
        for (given, kept) in [
            ("Relevé 2026-10.pdf", "Relevé 2026-10.pdf"),
            ("../../.bashrc", "_._.bashrc"),
            ("..", NAMELESS),
            ("", NAMELESS),
            ("   ", NAMELESS),
            (".hidden", "hidden"),
            ("...", NAMELESS),
            ("a/b\\c.txt", "a_b_c.txt"),
            ("/etc/passwd", "_etc_passwd"),
            ("C:\\Windows\\x.exe", "C__Windows_x.exe"),
            ("report..final.pdf", "report.final.pdf"),
            ("what?<now>|\"x\"*.pdf", "what__now___x__.pdf"),
            ("tab\there\nnew.pdf", "tabherenew.pdf"),
            ("trailing. . ", "trailing"),
            ("con.txt", "_con.txt"),
            ("NUL", "_NUL"),
            ("com1.log", "_com1.log"),
            ("company.pdf", "company.pdf"),
            // A program dressed as a PDF by a right-to-left mark reads as what it is.
            ("invoice\u{202E}fdp.exe", "invoicefdp.exe"),
            ("zero\u{200B}width.pdf", "zerowidth.pdf"),
            // Invisible fillers and separators, a soft hyphen, the tags: gone.
            ("line\u{2028}break\u{2029}.pdf", "linebreak.pdf"),
            ("soft\u{00AD}hyphen\u{180E}\u{3164}\u{115F}\u{1160}\u{FFA0}\u{2800}.pdf", "softhyphen.pdf"),
            ("tag\u{E0041}\u{E007F}.pdf", "tag.pdf"),
            // A long gap cannot push a program's extension out of a line.
            ("statement.pdf                                                                                                    .exe", "statement.pdf .exe"),
            ("non\u{00A0}\u{00A0}breaking.pdf", "non breaking.pdf"),
            // Only an extension: a name, its type kept; a dotfile stays a plain name.
            (".pdf", "download.pdf"),
            (".bashrc", "bashrc"),
            // Windows' devices with superscript digits.
            ("COM\u{00B9}.txt", "_COM\u{00B9}.txt"),
            ("lpt\u{00B3}", "_lpt\u{00B3}"),
            ("com0.txt", "_com0.txt"),
        ] {
            assert_eq!(clean_name(given), kept, "{given:?}");
            assert_eq!(clean_name(kept), kept, "cleaning again changes nothing: {kept:?}");
        }
        // A long name cut down to a device's is caught too, and cleaning it again changes nothing.
        let device = format!("COM1{}x.txt", "\u{2003}".repeat(300));
        assert_eq!(clean_name(&device), "COM1 x.txt");
        let device = format!("COM1.{}", "c".repeat(300));
        let cut = clean_name(&device);
        assert!(cut.starts_with("_COM1") && cut.chars().count() <= NAME_CHARS, "{cut}");
        assert_eq!(clean_name(&cut), cut);
        let long = format!("{}.pdf", "a".repeat(400));
        let cut = clean_name(&long);
        assert_eq!(cut.chars().count(), NAME_CHARS);
        assert!(cut.ends_with(".pdf"));
        // Bytes as well as characters: a name in a script of three bytes a letter.
        let wide = format!("{}.pdf", "关".repeat(100));
        let cut = clean_name(&wide);
        assert!(cut.len() <= NAME_BYTES && cut.ends_with(".pdf"), "{} bytes", cut.len());
        // An "extension" too long to be one is part of the name, cut as such.
        let odd = format!("x.{}", "b".repeat(300));
        assert!(clean_name(&odd).chars().count() <= NAME_CHARS);
        // Whatever comes in, what comes out is one name in its folder.
        for given in ["../a", "a/../b", "./x", "x/.", "\\\\server\\share", "....//....//etc"] {
            let name = clean_name(given);
            assert!(!name.contains(['/', '\\']) && !name.contains("..") && !name.starts_with('.') && !name.is_empty(), "{given:?} → {name:?}");
            assert_eq!(Path::new("/folder").join(&name).parent(), Some(Path::new("/folder")), "{given:?}");
        }
    }

    /// A name already there gets a number, in the place's own way.
    /// A name without an extension gets one from the type the site says, for
    /// the types Sioul shows; Sioul's own lists are known by their names.
    #[test]
    fn a_name_gets_its_type_s_extension() {
        assert_eq!(file_name("statement", "application/pdf"), "statement.pdf");
        assert_eq!(file_name("", "application/PDF; charset=binary"), "download.pdf");
        assert_eq!(file_name("budget", "application/vnd.oasis.opendocument.spreadsheet"), "budget.ods");
        assert_eq!(file_name("scan.png", "application/pdf"), "scan.png", "an extension given is kept");
        assert_eq!(file_name("blob", "application/octet-stream"), "blob", "no type Sioul shows: none");
        assert!(has_extension("a.pdf") && !has_extension("statement") && !has_extension("x.not an extension"));
        for own in ["sioul-papers.toml", "SIOUL-BANK.TOML", "projects.toml", "sioul-cases.toml.before-rename", "notes.sioul-move"] {
            assert!(is_sioul_own(own), "{own}");
        }
        assert!(!is_sioul_own("statement.pdf") && !is_sioul_own("toml.pdf"));
    }

    #[test]
    fn a_name_taken_gets_its_number() {
        assert_eq!(numbered("statement.pdf", 1, Numbering::Paren), "statement.pdf");
        assert_eq!(numbered("statement.pdf", 2, Numbering::Paren), "statement (2).pdf");
        assert_eq!(numbered("statement.pdf", 3, Numbering::Dash), "statement-3.pdf");
        assert_eq!(numbered("statement.pdf", 2, Numbering::Space), "statement 2.pdf");
        assert_eq!(numbered("README", 2, Numbering::Paren), "README (2)");
        assert_eq!(Place::Papers.numbering(), Numbering::Dash);
        assert_eq!(Place::Notes.numbering(), Numbering::Space);
        assert_eq!(Place::Letters.numbering(), Numbering::Paren);
    }

    #[test]
    fn programs_by_their_name_or_their_type() {
        assert!(is_program("setup.exe", ""));
        assert!(is_program("tool.AppImage", "application/octet-stream"));
        assert!(is_program("app.apk", ""));
        assert!(is_program("statement", "application/x-msdownload"));
        assert!(is_program("statement.pdf", "application/x-sh; charset=utf-8"));
        assert!(!is_program("statement.pdf", "application/pdf"));
        assert!(!is_program("photo.jpg", "image/jpeg"));
    }

    #[test]
    fn the_host_a_file_comes_from() {
        assert_eq!(host_of("https://secure.bank.example/statements/1?x=2"), "secure.bank.example");
        assert_eq!(host_of("blob:https://secure.bank.example/0f8e-11"), "secure.bank.example");
        assert_eq!(host_of("https://user:secret@Files.Example.org:8443/a"), "files.example.org");
        assert_eq!(host_of("http://[::1]:8080/x"), "[::1]");
        assert_eq!(host_of("data:text/plain,hello"), "");
        assert_eq!(host_of("file:///home/someone/x.pdf"), "");
        assert_eq!(host_of("javascript:alert(1)"), "");
    }

    #[test]
    fn words_from_a_site_cut_in_their_middle() {
        assert_eq!(shortened("short.pdf", 20), "short.pdf");
        let cut = shortened("a-very-long-name-for-a-statement-2026-10.pdf", 21);
        assert_eq!(cut.chars().count(), 21);
        assert!(cut.starts_with("a-very-lon") && cut.ends_with("10.pdf") && cut.contains('…'), "{cut}");
    }

    /// The place chosen last for a site is offered first for that site;
    /// Downloads the first time; a folder kept for the system's window.
    #[test]
    fn the_last_place_is_remembered_per_site() {
        let dir = std::env::temp_dir().join(format!("sioul-downloads-memory-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("downloads.toml");
        let mut memory = Memory::load(&path);
        assert_eq!(memory.first("bank"), (Place::Downloads, String::new()), "Downloads the first time");
        memory.chose("bank", Place::Papers, "");
        memory.chose("chat", Place::Folder, "/stand-in/Shared");
        memory.save(&path).unwrap();
        let read = Memory::load(&path);
        assert_eq!(read.first("bank"), (Place::Papers, String::new()));
        assert_eq!(read.first("chat"), (Place::Folder, "/stand-in/Shared".to_string()));
        assert_eq!(read.first("taxes"), (Place::Downloads, String::new()), "another site has its own");
        // The folder chosen once is kept while another place is chosen, for the next time the window opens.
        let mut read = read;
        read.chose("chat", Place::Notes, "");
        assert_eq!(read.site["chat"], Chosen { place: "notes".into(), folder: "/stand-in/Shared".into() });
        // A file written by a newer Sioul with a place this one does not know: Downloads.
        std::fs::write(&path, "[site.bank]\nplace = \"elsewhere\"\n").unwrap();
        assert_eq!(Memory::load(&path).first("bank").0, Place::Downloads);
        std::fs::write(&path, "not toml [").unwrap();
        assert_eq!(Memory::load(&path), Memory::default(), "a file that does not read: nothing remembered");
        assert_eq!(site_key("", "files.example.org"), "host:files.example.org");
        assert_eq!(site_key("bank", "secure.bank.example"), "bank");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

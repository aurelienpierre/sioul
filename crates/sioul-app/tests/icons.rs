// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Every icon the window names is in Sioul's bundled icon themes, the set for
//! light colours and the set for dark, as Qt itself reads them with no theme
//! of the system's to fall back on; and every icon of the bundle draws
//! something. Windows, macOS and phones have no icon theme: a name the bundle
//! lacks shows nothing there, while a Linux desktop draws it from its own
//! theme and nobody notices. Here it fails, on each system of the CI
//! (.github/workflows/build.yml).
//!
//! The names are read from the window's sources: what the QML gives to
//! `icon.name`, `iconName` and `icon`, and to an `IconImage`'s `name`; the
//! values of the tables and functions named for icons (`kindIcons`,
//! `laneIcon`); and those the Rust code hands the window (the weather's, the
//! mail folders'). A name given any other way is not seen: give it so.
//!
//! Its own `main` (harness = false): Qt wants its application on the
//! program's first thread, and the environment is set before anything reads
//! it. Run it alone: `cargo test --release -p sioul-app --test icons`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

fn main() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    // No theme of the system's: no platform theme of the desktop's (KDE's
    // would draw any name from Breeze), no system icon theme named, the XDG
    // folders an empty one, Qt drawing offscreen. SIOUL_THEME chooses the
    // colours, and the other XDG folders are that empty one too, so that
    // nothing of yours is read.
    let empty = std::env::temp_dir().join(format!("sioul-icons-test-{}", std::process::id()));
    std::fs::create_dir_all(&empty).expect("an empty folder for the XDG folders");
    // SAFETY: nothing else runs yet: no thread of this program reads the environment.
    unsafe {
        std::env::set_var("QT_QPA_PLATFORM", "offscreen");
        std::env::remove_var("QT_QPA_PLATFORMTHEME");
        std::env::remove_var("QT_QPA_SYSTEM_ICON_THEME");
        for folder in ["XDG_DATA_HOME", "XDG_DATA_DIRS", "XDG_CONFIG_HOME", "XDG_STATE_HOME", "XDG_CACHE_HOME"] {
            std::env::set_var(folder, &empty);
        }
        std::env::set_var("SIOUL_THEME", "light");
    }
    let application = cxx_qt_lib::QGuiApplication::new();
    assert!(!application.is_null(), "Qt's application, offscreen");

    let mut wrong: Vec<String> = Vec::new();
    let used = names_used(crate_dir);
    let names: Vec<String> = used.keys().cloned().collect();
    // Light colours as Sioul starts, then dark, then light again, as the window
    // asks when its colours change: the set in use follows each time.
    for dark in [false, true, false] {
        let set = if dark { "sioul-dark" } else { "sioul" };
        let lacking = sioul_app::icons_lacking(&names, dark);
        let in_use = sioul_app::icon_theme();
        if in_use != set {
            wrong.push(format!("the window's colours {}: the icon theme in use is \"{in_use}\", not \"{set}\"", if dark { "dark" } else { "light" }));
        }
        for name in lacking {
            let at = used.get(&name).map(|places| places.iter().take(3).cloned().collect::<Vec<_>>().join(", ")).unwrap_or_default();
            wrong.push(format!("{set} lacks \"{name}\" (named at {at}): tools/bundle-icons.py copies it from Breeze, if Breeze draws it"));
        }
    }
    wrong.extend(bundle_draws(crate_dir));
    let _ = std::fs::remove_dir_all(&empty);

    if wrong.is_empty() {
        println!("icons: {} names, each in both bundled sets; every bundled icon draws", names.len());
    } else {
        eprintln!("icons: {} problems\n{}", wrong.len(), wrong.join("\n"));
        std::process::exit(1);
    }
}

/// Every icon name the window uses, with where it is named.
fn names_used(crate_dir: &Path) -> BTreeMap<String, BTreeSet<String>> {
    let mut used: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut files = Vec::new();
    for (folder, extension) in [("qml", "qml"), ("qml-desktop", "qml"), ("src", "js")] {
        files_in(&crate_dir.join(folder), extension, &mut files);
    }
    files.sort();
    for file in &files {
        let text = std::fs::read_to_string(file).expect("a QML or JavaScript file of the window");
        let shown = file.strip_prefix(crate_dir).unwrap_or(file).display().to_string();
        for (line, name) in names_in(&text) {
            used.entry(name).or_default().insert(format!("{shown}:{line}"));
        }
    }
    // The weather's icons, for every code and by day and night (sioul-core, weather.rs).
    for code in 0..=u8::MAX {
        for day in [true, false] {
            used.entry(sioul_core::weather::describe(code, day).1.to_string()).or_default().insert("sioul-core weather::describe".into());
        }
    }
    // The mail folders' (sioul-core, folders.rs).
    use sioul_core::folders::Role;
    for role in [Role::Inbox, Role::Drafts, Role::Sent, Role::Archive, Role::Junk, Role::Trash, Role::All, Role::Other] {
        // A role added to `Role` stops this from compiling until it is listed above.
        match role {
            Role::Inbox | Role::Drafts | Role::Sent | Role::Archive | Role::Junk | Role::Trash | Role::All | Role::Other => {}
        }
        used.entry(role.icon().to_string()).or_default().insert("sioul-core Role::icon".into());
    }
    used
}

fn files_in(folder: &Path, extension: &str, found: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(folder) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            files_in(&path, extension, found);
        } else if path.extension().is_some_and(|e| e == extension) {
            found.push(path);
        }
    }
}

/// The icon names a QML or JavaScript file gives, with their line.
fn names_in(text: &str) -> Vec<(usize, String)> {
    let mut found = Vec::new();
    // Inside an `IconImage { … }`, and inside a table or function named for icons: bracket depths.
    let (mut image, mut table) = (0i32, 0i32);
    for (index, raw) in text.lines().enumerate() {
        let line = code_of(raw);
        let mut values: Vec<&str> = Vec::new();
        for key in ["icon.name", "iconName", "icon"] {
            values.extend(after_keys(line, key).into_iter().map(value_of));
        }
        if image > 0 {
            values.extend(after_keys(line, "name").into_iter().map(value_of));
        }
        if image > 0 || opens_icon_image(line) {
            image = (image + depth_change(line)).max(0);
        }
        if table > 0 || names_icons(line) {
            values.push(line);
            table = (table + depth_change(line)).max(0);
        }
        for value in values {
            found.extend(literals_in(value).into_iter().map(|name| (index + 1, name)));
        }
    }
    found
}

/// A line without its comment ("//" outside a string).
fn code_of(line: &str) -> &str {
    let mut quoted = false;
    let bytes = line.as_bytes();
    for i in 0..bytes.len() {
        match bytes[i] {
            b'"' if i == 0 || bytes[i - 1] != b'\\' => quoted = !quoted,
            b'/' if !quoted && bytes.get(i + 1) == Some(&b'/') => return &line[..i],
            _ => {}
        }
    }
    line
}

/// What follows each `key:` of the line (`key` a whole word, the colon after spaces at most).
fn after_keys<'a>(line: &'a str, key: &str) -> Vec<&'a str> {
    let word = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let mut found = Vec::new();
    for (at, _) in line.match_indices(key) {
        if line[..at].chars().next_back().is_some_and(word) {
            continue;
        }
        let rest = &line[at + key.len()..];
        if rest.starts_with(|c: char| word(c) || c == '.') {
            continue;
        }
        if let Some(value) = rest.trim_start().strip_prefix(':') {
            found.push(value);
        }
    }
    found
}

/// A property's value or an object's member: up to the first comma or
/// closing bracket that is not inside brackets or a string.
fn value_of(text: &str) -> &str {
    let (mut depth, mut quoted) = (0, false);
    for (i, c) in text.char_indices() {
        match c {
            '"' => quoted = !quoted,
            _ if quoted => {}
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' if depth == 0 => return &text[..i],
            ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => return &text[..i],
            _ => {}
        }
    }
    text
}

/// The brackets a line opens, less those it closes (outside strings).
fn depth_change(line: &str) -> i32 {
    let (mut change, mut quoted) = (0, false);
    for c in line.chars() {
        match c {
            '"' => quoted = !quoted,
            _ if quoted => {}
            '(' | '[' | '{' => change += 1,
            ')' | ']' | '}' => change -= 1,
            _ => {}
        }
    }
    change
}

fn opens_icon_image(line: &str) -> bool {
    line.split_once("IconImage").is_some_and(|(before, after)| !before.ends_with(|c: char| c.is_ascii_alphanumeric()) && after.trim_start().starts_with('{'))
}

/// A table or a function named for icons: `property var kindIcons:`, `function laneIcon(`.
fn names_icons(line: &str) -> bool {
    let named = |rest: &str, end: char| rest.split(end).next().is_some_and(|name| {
        let name = name.trim();
        !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') && (name.ends_with("Icon") || name.ends_with("Icons"))
    });
    line.split_once("function ").is_some_and(|(_, rest)| rest.contains('(') && named(rest, '('))
        || line.split_once("property var ").is_some_and(|(_, rest)| rest.contains(':') && named(rest, ':'))
}

/// The strings of `text` shaped as icon names ("mail-reply-sender"), but for
/// an object's keys, what is compared, and what is handed to a call (a
/// message, a prefix): `{ "verified": "security-high" }[level] || "security-medium"`.
fn literals_in(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = text;
    let mut offset = 0;
    while let Some(start) = rest.find('"') {
        let Some(length) = rest[start + 1..].find('"') else { break };
        let (open, close) = (offset + start, offset + start + 1 + length);
        let literal = &text[open + 1..close];
        let before = text[..open].trim_end();
        let after = &text[close + 1..];
        let compared = ["===", "!==", "==", "!="].iter().any(|op| before.ends_with(op) || after.trim_start().starts_with(op));
        let handed = [".startsWith(", ".endsWith(", ".includes(", ".indexOf(", ".text(", ".textWith("].iter().any(|call| before.ends_with(call));
        if shaped(literal) && !after.starts_with(':') && !compared && !handed {
            found.push(literal.to_string());
        }
        offset = close + 1;
        rest = &text[offset..];
    }
    found
}

/// Lower case words joined by hyphens, as freedesktop names icons.
fn shaped(name: &str) -> bool {
    !name.is_empty()
        && name.starts_with(|c: char| c.is_ascii_lowercase())
        && name.split('-').all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()))
}

/// Every icon of the bundle (icons.qrc) is a file of icons/ and draws
/// something, read as the window reads it (Qt's SVG image format); every SVG
/// of icons/ is listed in icons.qrc.
fn bundle_draws(crate_dir: &Path) -> Vec<String> {
    let icons = crate_dir.join("icons");
    let qrc = std::fs::read_to_string(icons.join("icons.qrc")).expect("icons/icons.qrc");
    let listed: BTreeSet<String> = qrc.split("<file>").skip(1).filter_map(|part| part.split_once("</file>").map(|(file, _)| file.to_string())).collect();
    let mut wrong = Vec::new();
    let mut on_disk = Vec::new();
    files_in(&icons, "svg", &mut on_disk);
    for path in on_disk {
        let relative = path.strip_prefix(&icons).unwrap_or(&path).components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/");
        if !listed.contains(&relative) {
            wrong.push(format!("icons/{relative} is not listed in icons.qrc: tools/bundle-icons.py lists every icon"));
        }
    }
    for file in listed.iter().filter(|file| file.ends_with(".svg")) {
        let Ok(bytes) = std::fs::read(icons.join(file)) else {
            wrong.push(format!("icons.qrc lists icons/{file}, which is not there"));
            continue;
        };
        match cxx_qt_lib::QImage::from_data(&bytes, Some("svg")) {
            None => wrong.push(format!("icons/{file} does not read as an SVG image (is Qt's SVG image plugin there?)")),
            Some(image) => {
                let drawn = (0..image.height()).any(|y| (0..image.width()).any(|x| image.pixel_color(x, y).alpha() > 0));
                if !drawn {
                    wrong.push(format!("icons/{file} draws nothing"));
                }
            }
        }
    }
    wrong
}

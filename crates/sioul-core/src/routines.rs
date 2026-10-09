// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Routines (docs/tasks.md, "Routines"): a sequence of timed steps played one
//! at a time, the next one said before it comes, so nothing is decided while
//! doing (Routinery). Moving on when a step's time is up is each routine's
//! choice: it helps some and pressures others. Skipping costs nothing and is
//! never counted. Kept in the configuration (`[[routine]]`), so they travel
//! with your settings.

use serde::{Deserialize, Serialize};

/// One step: what, for how long, and what it opens (`porch`, `sioul:task/<UID>`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Step {
    pub title: String,
    #[serde(default)]
    pub minutes: u32,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub open: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Routine {
    pub id: String,
    pub title: String,
    /// The next step starts by itself when a step's time is up.
    #[serde(default)]
    pub auto: bool,
    #[serde(default, rename = "step")]
    pub steps: Vec<Step>,
}

/// "10 min Open the Porch", "Open the Porch 10", "Open the Porch ~10m", "Make tea (5)": one step a line.
pub fn parse_steps(text: &str) -> Vec<Step> {
    text.lines().map(str::trim).filter(|l| !l.is_empty()).map(parse_step).collect()
}

fn minutes_word(word: &str) -> Option<u32> {
    let word = word.trim_matches(|c: char| c == '~' || c == '(' || c == ')' || c == ',');
    let lower = word.to_lowercase();
    let digits: String = lower.chars().take_while(char::is_ascii_digit).collect();
    let rest = &lower[digits.len()..];
    let number: u32 = digits.parse().ok()?;
    match rest {
        "" | "m" | "min" | "mn" | "minutes" | "minute" => Some(number),
        "h" => number.checked_mul(60),
        _ => None,
    }
}

fn parse_step(line: &str) -> Step {
    let words: Vec<&str> = line.split_whitespace().collect();
    // At the start: "10 min …", "10min …", "~10m …".
    if let Some(first) = words.first().and_then(|w| minutes_word(w)) {
        let skip = if words.get(1).is_some_and(|w| matches!(w.to_lowercase().as_str(), "min" | "mn" | "minutes" | "minute")) { 2 } else { 1 };
        let title = words[skip.min(words.len())..].join(" ");
        if !title.is_empty() {
            return Step { title, minutes: first, open: String::new() };
        }
    }
    // At the end: "… 10", "… 10 min", "… ~10m", "… (5)".
    let n = words.len();
    if n >= 2 && matches!(words[n - 1].to_lowercase().as_str(), "min" | "mn" | "minutes" | "minute")
        && let Some(minutes) = minutes_word(words[n - 2])
    {
        return Step { title: words[..n - 2].join(" "), minutes, open: String::new() };
    }
    if n >= 2
        && let Some(minutes) = minutes_word(words[n - 1])
    {
        return Step { title: words[..n - 1].join(" "), minutes, open: String::new() };
    }
    Step { title: line.to_string(), minutes: 5, open: String::new() }
}

/// The steps as one line each, as the form shows them: "10 min Open the Porch".
pub fn steps_text(steps: &[Step]) -> String {
    steps.iter().map(|s| format!("{} min {}", s.minutes, s.title)).collect::<Vec<_>>().join("\n")
}

/// The routines as the configuration holds them now, changed by `change` and
/// written back, the whole under the file's lock, which the sharing takes
/// too: a routine another device's change brought in meanwhile is never
/// written over with an older list.
pub fn change(config_path: &std::path::Path, change: impl FnOnce(&mut Vec<Routine>)) -> Result<(), String> {
    #[derive(Deserialize, Default)]
    struct Listed {
        #[serde(default)]
        routine: Vec<Routine>,
    }
    crate::filelock::with_lock(config_path, || {
        let fail = |e: String| format!("{}: {e}", config_path.display());
        let text = match std::fs::read_to_string(config_path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(fail(e.to_string())),
        };
        let mut routines = toml::from_str::<Listed>(&text).map_err(|e| fail(e.to_string()))?.routine;
        change(&mut routines);
        save(config_path, &routines)
    })
}

/// The routines written back into the configuration, the rest of it as
/// written, read and written under the file's lock.
pub fn save(config_path: &std::path::Path, routines: &[Routine]) -> Result<(), String> {
    crate::filelock::with_lock(config_path, || save_locked(config_path, routines))
}

fn save_locked(config_path: &std::path::Path, routines: &[Routine]) -> Result<(), String> {
    let fail = |e: String| format!("{}: {e}", config_path.display());
    // No configuration yet: it is made. One there but unreadable (its rights, an
    // encoding other than UTF-8) or not TOML is left alone: written over, every setting would go.
    let text = match std::fs::read_to_string(config_path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(fail(e.to_string())),
    };
    let mut doc: toml_edit::DocumentMut = text.parse().map_err(|e: toml_edit::TomlError| fail(e.to_string()))?;
    doc.remove("routine");
    if !routines.is_empty() {
        let mut list = toml_edit::ArrayOfTables::new();
        for routine in routines {
            let mut table = toml_edit::Table::new();
            table["id"] = toml_edit::value(&routine.id);
            table["title"] = toml_edit::value(&routine.title);
            if routine.auto {
                table["auto"] = toml_edit::value(true);
            }
            let mut steps = toml_edit::ArrayOfTables::new();
            for step in &routine.steps {
                let mut s = toml_edit::Table::new();
                s["title"] = toml_edit::value(&step.title);
                s["minutes"] = toml_edit::value(i64::from(step.minutes));
                if !step.open.is_empty() {
                    s["open"] = toml_edit::value(&step.open);
                }
                steps.push(s);
            }
            table.insert("step", toml_edit::Item::ArrayOfTables(steps));
            list.push(table);
        }
        doc.insert("routine", toml_edit::Item::ArrayOfTables(list));
    }
    let temporary = config_path.with_extension("toml.new");
    std::fs::write(&temporary, doc.to_string()).and_then(|()| std::fs::rename(&temporary, config_path)).map_err(|e| format!("{}: {e}", config_path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_read_from_lines() {
        let steps = parse_steps("10 min Open the Porch\nAnswer the two cards 20\nFile ~5m\nMake tea (3)\n\n2h Deep clean\nJust this");
        let pairs: Vec<(&str, u32)> = steps.iter().map(|s| (s.title.as_str(), s.minutes)).collect();
        assert_eq!(pairs, vec![("Open the Porch", 10), ("Answer the two cards", 20), ("File", 5), ("Make tea", 3), ("Deep clean", 120), ("Just this", 5)]);
        assert_eq!(parse_steps(&steps_text(&steps)), steps);
    }

    #[test]
    fn saved_in_the_configuration() {
        let dir = std::env::temp_dir().join(format!("sioul-routines-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        std::fs::write(&path, "# mine\nlanguage = \"fr\"\n").unwrap();
        let routine = Routine { id: "morning".into(), title: "Morning".into(), auto: true, steps: parse_steps("5 min Water\n10 min Stretch") };
        save(&path, std::slice::from_ref(&routine)).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.starts_with("# mine\nlanguage = \"fr\""), "{text}");
        let config: crate::config::Config = toml::from_str(&text).unwrap();
        assert_eq!(config.routines, vec![routine.clone()]);
        save(&path, &[]).unwrap();
        assert!(!std::fs::read_to_string(&path).unwrap().contains("routine"));
        // A configuration that cannot be read (here, not UTF-8) or is not TOML is never written over.
        for broken in [b"# mine\nlanguage = \"fran\xe7ais\"\n".to_vec(), b"language = = \"fr\"\n".to_vec()] {
            std::fs::write(&path, &broken).unwrap();
            assert!(save(&path, std::slice::from_ref(&routine)).is_err());
            assert_eq!(std::fs::read(&path).unwrap(), broken);
        }
        assert_eq!(parse_steps("Rest 99999999h"), vec![Step { title: "Rest 99999999h".into(), minutes: 5, open: String::new() }], "too long: no time read");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

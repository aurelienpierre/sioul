// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Quick capture: one line typed, a task made. What the line says besides
//! the title is shown as chips before the task is made, so nothing is misread
//! silently (docs/tasks.md).
//!
//! At the end of the line, in the languages in use (`words::CaptureWords`;
//! English and French built in), in any order:
//! - **the day to start**: "today", "tomorrow", "friday", "next week",
//!   "in 3 days", "30/10", "30 oct", "2026-10-30" ("aujourd'hui", "demain",
//!   "vendredi", "semaine prochaine", "dans 3 jours", "30 octobre");
//! - **how long**: "~15m", "~1h", "~1h30";
//! - **the date asked**, in braces: "{30/10}", "{vendredi}";
//! - **a project or a tag**: "#housing".
//!
//! Anywhere in the line, "@tomorrow" or "@30/10" gives the day to start, and
//! "@call", "@write", "@online", "@out", "@read", "@think", "@make" (or in
//! French: "@appel", "@écrire", "@enligne", "@dehors", "@lire",
//! "@réfléchir", "@faire") the task's kind, as does "@" and the name of a
//! kind you added ("@errand"). Without one, a first word that says it
//! plainly gives it: "Call the bank" is a call, "Remplir le formulaire" a
//! form online. A kind you took away is never given. Words are compared
//! folded: capitals and accents aside ("fevrier", "Écrire").

use crate::tasks::TaskEdit;
use crate::words::{CaptureWords, TaskKinds, folded};
use jiff::Span;
use jiff::civil::{Date, Weekday};
use serde::Serialize;

/// Something the line said besides the title.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Chip {
    /// "start", "due", "estimate", "project", "tag", "kind".
    pub kind: String,
    /// As typed.
    pub text: String,
    /// As understood: "2026-10-30", "15", "housing".
    pub value: String,
}

/// A captured line: the task it makes, and the chips to show.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Captured {
    pub edit: TaskEdit,
    pub chips: Vec<Chip>,
}

/// Whether a folded word or phrase is one of a list's, folded, its spaces made one.
fn said(list: &[String], phrase: &str) -> bool {
    list.iter().any(|p| {
        let p = folded(p);
        !p.is_empty() && p.split_whitespace().collect::<Vec<_>>().join(" ") == phrase
    })
}

fn weekday(w: &CaptureWords, word: &str) -> Option<Weekday> {
    let d = &w.weekdays;
    [
        (&d.monday, Weekday::Monday),
        (&d.tuesday, Weekday::Tuesday),
        (&d.wednesday, Weekday::Wednesday),
        (&d.thursday, Weekday::Thursday),
        (&d.friday, Weekday::Friday),
        (&d.saturday, Weekday::Saturday),
        (&d.sunday, Weekday::Sunday),
    ]
    .into_iter()
    .find(|(list, _)| said(list, word))
    .map(|(_, day)| day)
}

/// A month by one of its names (`CaptureWords::months`, by number), a dot after it or not.
fn month(w: &CaptureWords, word: &str) -> Option<i8> {
    let word = word.trim_end_matches('.');
    w.months.iter().find(|(_, names)| said(names, word)).and_then(|(number, _)| number.trim().parse().ok()).filter(|m| (1..=12).contains(m))
}

/// None past what a calendar holds ("in 99999999 days"), where `Span::days` would panic.
fn add_days(date: Date, days: i64) -> Option<Date> {
    date.checked_add(Span::new().try_days(days).ok()?).ok()
}

/// A day and month without a year: the next one from today.
fn next_date(today: Date, month: i8, day: i8, year: Option<i16>) -> Option<Date> {
    let year = year.map(|y| if y < 100 { 2000 + y } else { y });
    let date = Date::new(year.unwrap_or(today.year()), month, day).ok()?;
    if year.is_none() && date < today { Date::new(today.year() + 1, month, day).ok() } else { Some(date) }
}

/// A day, as typed: one to three words, in the languages in use (`words::current`).
pub fn parse_day(text: &str, today: Date) -> Option<Date> {
    parse_day_with(&crate::words::current().capture, text, today)
}

/// A day, as typed, read with these words.
pub fn parse_day_with(w: &CaptureWords, text: &str, today: Date) -> Option<Date> {
    let lower = folded(text);
    let words: Vec<&str> = lower.split_whitespace().collect();
    let phrase = words.join(" ");
    if said(&w.today, &phrase) {
        return Some(today);
    }
    if said(&w.tomorrow, &phrase) {
        return add_days(today, 1);
    }
    if said(&w.next_week, &phrase) {
        let to_monday = 7 - i64::from(today.weekday().to_monday_zero_offset());
        return add_days(today, to_monday);
    }
    if let [first, n, unit] = words.as_slice()
        && said(&w.in_, first)
    {
        let n: i64 = n.parse().ok()?;
        let days = if said(&w.unit_days, unit) {
            n
        } else if said(&w.unit_weeks, unit) {
            n.checked_mul(7)?
        } else {
            return None;
        };
        return add_days(today, days);
    }
    match words.as_slice() {
        [word] => {
            if let Some(day) = weekday(w, word) {
                // The next one: a Friday said on a Friday is a week away.
                let ahead = (i64::from(day.to_monday_zero_offset()) - i64::from(today.weekday().to_monday_zero_offset()) + 7) % 7;
                return add_days(today, if ahead == 0 { 7 } else { ahead });
            }
            if let Ok(date) = word.parse::<Date>() {
                return Some(date);
            }
            let parts: Vec<&str> = word.split(['/', '.']).collect();
            if let [d, m, rest @ ..] = parts.as_slice()
                && rest.len() <= 1
            {
                let year = rest.first().and_then(|y| y.parse::<i16>().ok());
                return next_date(today, m.parse().ok()?, d.parse().ok()?, year);
            }
        }
        [a, b] | [a, b, _] => {
            let year = words.get(2).and_then(|y| y.parse::<i16>().ok());
            if words.len() == 3 && year.is_none() {
                return None;
            }
            // "1er", "2nd": the day's ordinal suffixes off (`CaptureWords::ordinals`).
            let number = |word: &str| {
                let mut word = word;
                for suffix in w.ordinals.iter().map(|o| folded(o)).filter(|o| !o.is_empty()) {
                    word = word.trim_end_matches(suffix.as_str());
                }
                word.parse::<i8>().ok()
            };
            if let (Some(day), Some(m)) = (number(a), month(w, b)) {
                return next_date(today, m, day, year);
            }
            if let (Some(m), Some(day)) = (month(w, a), number(b)) {
                return next_date(today, m, day, year);
            }
        }
        _ => {}
    }
    None
}

/// "~15m", "~1h", "~1h30", "~90" → minutes.
pub fn parse_minutes(text: &str) -> Option<u32> {
    let text = text.trim().trim_start_matches('~').to_lowercase();
    let text = text.trim_end_matches("min").trim_end_matches('m');
    if let Some((hours, minutes)) = text.split_once('h') {
        let hours: u32 = hours.parse().ok()?;
        let minutes: u32 = if minutes.is_empty() { 0 } else { minutes.parse().ok()? };
        return hours.checked_mul(60)?.checked_add(minutes).filter(|&m| m > 0);
    }
    text.parse().ok().filter(|&m: &u32| m > 0)
}

/// Reads one typed line. `projects` are the project ids a "#word" may name; any
/// other "#word" is a tag (CATEGORIES). `kinds` are your kinds, id and name;
/// empty, Sioul's. The words are those in use (`words::current`).
pub fn capture(line: &str, today: Date, projects: &[String], kinds: &[(String, String)]) -> Captured {
    capture_with(&crate::words::current().capture, line, today, projects, kinds)
}

/// `capture`, read with these words.
pub fn capture_with(w: &CaptureWords, line: &str, today: Date, projects: &[String], kinds: &[(String, String)]) -> Captured {
    let mut edit = TaskEdit::default();
    let mut chips: Vec<Chip> = Vec::new();
    let mut words: Vec<String> = line.split_whitespace().map(str::to_string).collect();
    let chip = |kind: &str, text: &str, value: String| Chip { kind: kind.into(), text: text.into(), value };

    // "@day" anywhere.
    words.retain(|word| {
        let Some(day) = word.strip_prefix('@').and_then(|d| parse_day_with(w, d, today)) else { return true };
        if edit.start.is_empty() {
            edit.start = day.to_string();
            chips.push(chip("start", word, day.to_string()));
            return false;
        }
        true
    });

    // "@kind" anywhere.
    words.retain(|word| {
        let Some(kind) = word.strip_prefix('@').and_then(|k| named_kind(&w.kind_words, k, kinds)) else { return true };
        if edit.kind.is_empty() {
            edit.kind = kind.clone();
            chips.push(chip("kind", word, kind));
            return false;
        }
        true
    });

    // Then from the end, as long as words say something.
    loop {
        let Some(last) = words.last().cloned() else { break };
        if last.starts_with('~')
            && let Some(minutes) = parse_minutes(&last)
            && edit.estimate == 0
        {
            edit.estimate = minutes;
            chips.push(chip("estimate", &last, minutes.to_string()));
            words.pop();
            continue;
        }
        if let Some(tag) = last.strip_prefix('#').filter(|t| !t.is_empty()) {
            let is_project = projects.iter().any(|c| c.eq_ignore_ascii_case(tag));
            if is_project {
                edit.projects.push(tag.to_string());
            } else {
                edit.categories.push(tag.to_string());
            }
            chips.push(chip(if is_project { "project" } else { "tag" }, &last, tag.to_string()));
            words.pop();
            continue;
        }
        if last.ends_with('}') && edit.due.is_empty() {
            let open = words.iter().rposition(|w| w.starts_with('{'));
            if let Some(open) = open {
                let typed = words[open..].join(" ");
                if let Some(day) = parse_day_with(w, typed.trim_start_matches('{').trim_end_matches('}'), today) {
                    edit.due = day.to_string();
                    chips.push(chip("due", &typed, day.to_string()));
                    words.truncate(open);
                    continue;
                }
            }
        }
        if edit.start.is_empty() {
            let found = (1..=3usize).rev().filter(|&n| n <= words.len()).find_map(|n| {
                let typed = words[words.len() - n..].join(" ");
                parse_day_with(w, &typed, today).map(|day| (n, typed, day))
            });
            // A lone number is part of the title ("Room 12"), not a day.
            if let Some((n, typed, day)) = found.filter(|(n, typed, _)| *n > 1 || typed.contains(['/', '-', '.']) || !typed.chars().all(|c| c.is_ascii_digit())) {
                edit.start = day.to_string();
                chips.push(chip("start", &typed, day.to_string()));
                words.truncate(words.len() - n);
                continue;
            }
        }
        break;
    }
    chips.reverse();
    edit.title = words.join(" ");
    if edit.kind.is_empty()
        && let Some(kind) = words.first().and_then(|first| guess_kind(&w.kind_verbs, first)).filter(|k| kinds.is_empty() || kinds.iter().any(|(id, _)| id == k))
    {
        edit.kind = kind.to_string();
        chips.push(chip("kind", "", kind.to_string()));
    }
    Captured { edit, chips }
}

/// The kind a word after "@" names: one of Sioul's (`kind_words`, in the
/// languages in use) while you keep it; else one of yours, by its id or its name.
fn named_kind(kind_words: &TaskKinds, word: &str, kinds: &[(String, String)]) -> Option<String> {
    if let Some(kind) = kind_in(kind_words, word).filter(|k| kinds.is_empty() || kinds.iter().any(|(id, _)| id == k)) {
        return Some(kind.to_string());
    }
    let fold = |s: &str| crate::text::fold(s).into_iter().filter(|c| !c.is_whitespace()).collect::<String>();
    let wanted = fold(word);
    kinds.iter().find(|(id, label)| *id == wanted || fold(label) == wanted).map(|(id, _)| id.clone())
}

/// The kind of Sioul's (`tasks::KINDS`) whose words hold `word`, folded.
fn kind_in(kinds: &TaskKinds, word: &str) -> Option<&'static str> {
    let word = folded(word);
    [
        ("call", &kinds.call),
        ("write", &kinds.write),
        ("online", &kinds.online),
        ("out", &kinds.out),
        ("read", &kinds.read),
        ("think", &kinds.think),
        ("make", &kinds.make),
    ]
    .into_iter()
    .find(|(_, list)| said(list, &word))
    .map(|(kind, _)| kind)
}

/// The kind a first word says plainly (`kind_verbs`); none for a word that
/// could be several ("ask", "demander": a call or a message?).
fn guess_kind(kind_verbs: &TaskKinds, word: &str) -> Option<&'static str> {
    kind_in(kind_verbs, word.trim_end_matches([',', '.', ':', ';']))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Saturday 3 October 2026.
    fn today() -> Date {
        "2026-10-03".parse().unwrap()
    }

    fn day(text: &str) -> Option<String> {
        parse_day(text, today()).map(|d| d.to_string())
    }

    #[test]
    fn kinds_typed_or_plain() {
        let projects: Vec<String> = Vec::new();
        let typed = capture("Banque @appel demain ~15m", today(), &projects, &[]);
        assert_eq!((typed.edit.title.as_str(), typed.edit.kind.as_str(), typed.edit.start.as_str()), ("Banque", "call", "2026-10-04"));
        assert_eq!(capture("Remplir le formulaire de la CAF", today(), &projects, &[]).edit.kind, "online");
        assert_eq!(capture("Call the bank", today(), &projects, &[]).edit.kind, "call");
        assert_eq!(capture("Ask the lawyer", today(), &projects, &[]).edit.kind, "", "a word that could be several says nothing");
        assert_eq!(capture("Écrire au propriétaire", today(), &projects, &[]).edit.kind, "write");
        let both = capture("Call the bank @write", today(), &projects, &[]);
        assert_eq!(both.edit.kind, "write", "what is typed wins over the first word");
        assert_eq!(both.chips.iter().filter(|c| c.kind == "kind").count(), 1);
        // Your kinds: one added, "think" taken away.
        let yours = vec![("call".to_string(), "Call".to_string()), ("rendez-vous".to_string(), "Rendez-vous".to_string())];
        assert_eq!(capture("Dentist @rendez-vous", today(), &projects, &yours).edit.kind, "rendez-vous");
        assert_eq!(capture("Call the bank", today(), &projects, &yours).edit.kind, "call");
        assert_eq!(capture("Decide @think", today(), &projects, &yours).edit.kind, "", "a kind taken away is never given");
        assert_eq!(capture("Decide on the flat", today(), &projects, &yours).edit.kind, "");
    }

    #[test]
    fn days_in_both_languages() {
        assert_eq!(day("tomorrow").as_deref(), Some("2026-10-04"));
        assert_eq!(day("demain").as_deref(), Some("2026-10-04"));
        assert_eq!(day("vendredi").as_deref(), Some("2026-10-09"));
        assert_eq!(day("saturday").as_deref(), Some("2026-10-10"), "a Saturday said on a Saturday is a week away");
        assert_eq!(day("next week").as_deref(), Some("2026-10-05"));
        assert_eq!(day("semaine prochaine").as_deref(), Some("2026-10-05"));
        assert_eq!(day("dans 3 jours").as_deref(), Some("2026-10-06"));
        assert_eq!(day("30/10").as_deref(), Some("2026-10-30"));
        assert_eq!(day("2/1").as_deref(), Some("2027-01-02"), "past this year: next year");
        assert_eq!(day("30 octobre").as_deref(), Some("2026-10-30"));
        assert_eq!(day("1er novembre").as_deref(), Some("2026-11-01"));
        assert_eq!(day("Oct 30").as_deref(), Some("2026-10-30"));
        assert_eq!(day("2026-12-24").as_deref(), Some("2026-12-24"));
        assert_eq!(day("maybe"), None);
        assert_eq!((parse_minutes("~15m"), parse_minutes("~1h30"), parse_minutes("~2h"), parse_minutes("~x")), (Some(15), Some(90), Some(120), None));
        // Typed by mistake: nothing, and no crash.
        assert_eq!((day("in 99999999 days"), day("dans 9999999999999999999 semaines"), day("in 2000000000000000000 weeks")), (None, None, None));
        assert_eq!(parse_minutes("~99999999h"), None);
    }

    #[test]
    fn one_line_one_task() {
        let projects = vec!["housing".to_string()];
        let got = capture("Call the CAF tomorrow ~15m #housing {30 oct}", today(), &projects, &[]);
        assert_eq!(got.edit.title, "Call the CAF");
        assert_eq!((got.edit.start.as_str(), got.edit.due.as_str(), got.edit.estimate), ("2026-10-04", "2026-10-30", 15));
        assert_eq!((got.edit.projects.clone(), got.edit.categories.clone()), (vec!["housing".to_string()], vec![]));
        // "Call" says the kind plainly: its chip comes last, as nothing typed it.
        assert_eq!(got.chips.iter().map(|c| c.kind.as_str()).collect::<Vec<_>>(), vec!["start", "estimate", "project", "due", "kind"]);
        assert_eq!(got.edit.kind, "call");
        let tagged = capture("Prepare @lundi the tomorrow meeting notes #health", today(), &projects, &[]);
        assert_eq!((tagged.edit.title.as_str(), tagged.edit.start.as_str()), ("Prepare the tomorrow meeting notes", "2026-10-05"));
        assert_eq!(tagged.edit.categories, vec!["health"]);
        assert_eq!(capture("Book room 12", today(), &projects, &[]).edit.title, "Book room 12");
    }

    /// Words added in your configuration: German days, a German "in", a German verb.
    #[test]
    fn your_words_are_read() {
        let config: crate::config::Config = toml::from_str(
            "[words]\nlanguages = [\"fr\", \"en\"]\ncountries = [\"FR\"]\n[words.capture.tomorrow]\nadd = [\"morgen\"]\n[words.capture.weekdays.friday]\nadd = [\"Freitag\"]\n[words.capture.in]\nadd = [\"in\"]\n[words.capture.unit_days]\nadd = [\"Tagen\"]\n[words.capture.months]\nadd = { 3 = [\"März\"] }\n[words.capture.kind_verbs.call]\nadd = [\"anrufen\"]\n",
        )
        .unwrap();
        let yours = crate::words::Words::of(&config);
        let day = |text: &str| parse_day_with(&yours.capture, text, today()).map(|d| d.to_string());
        assert_eq!(day("morgen").as_deref(), Some("2026-10-04"));
        assert_eq!(day("FREITAG").as_deref(), Some("2026-10-09"));
        assert_eq!(day("in 3 Tagen").as_deref(), Some("2026-10-06"));
        assert_eq!(day("3 märz").as_deref(), Some("2027-03-03"));
        assert_eq!(day("3 marz").as_deref(), Some("2027-03-03"), "accents aside");
        assert_eq!(capture_with(&yours.capture, "Anrufen Bank morgen", today(), &[], &[]).edit.kind, "call");
        assert_eq!(parse_day("morgen", today()), None, "not in the packs");
        assert_eq!(capture("Anrufen Bank", today(), &[], &[]).edit.kind, "");
        // Accents typed or not, as before.
        assert_eq!((day("fevrier 2").as_deref(), day("2 févr.").as_deref(), day("Aout 15").as_deref()), (Some("2027-02-02"), Some("2027-02-02"), Some("2027-08-15")));
    }
}

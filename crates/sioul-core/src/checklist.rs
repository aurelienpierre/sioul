// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Checklists: GitHub task lists (GFM, "Task list items (extension)") in the
//! notes of a task or an event, its `DESCRIPTION` (docs/tasks.md,
//! "Checklists"). What to bring, what to check: lines with no identity of
//! their own, neither planned nor counted.
//!
//! ```text
//! Take the green folder, it holds the old results.
//!
//! Bring:
//! - [ ] Carte Vitale
//! - [x] Last blood test
//!   The paper one, in the green folder.
//! - [-] Vaccination record
//! ```
//!
//! - **An item** is a line: a bullet (`-`, `*`, `+`) or a number (`1.`,
//!   `1)`), a box, its words. `[ ]` is to do, `[x]` done, `[-]` not needed
//!   this time (the Obsidian Tasks plugin's "dropped": a convention, not
//!   GFM); neither done nor missed, nothing counts it. Lines in a fenced
//!   block of code are never items.
//! - **A group** is named by a line that ends with a colon just before items
//!   ("Bring:"); it lasts until the next words that are no item.
//! - **Its detail**: the lines indented under an item.
//! - **A tick** changes one character of one line, nothing else (`ticked`).
//! - **The free text and the list apart** (`free_text`): an agent rewriting
//!   the notes never wipes the list. Sioul writes a list in one form
//!   (`written`): the free text, a blank line, then the groups.
//! - **Two versions that differ only by their ticks** (two devices ticking
//!   different items) merge, every tick kept: done before not needed before
//!   to do (`merge_ticks`; a whole iCalendar file, `merge_calendar`). A tick
//!   is never lost.
//!
//! Other programs show the lines as they are: "- [ ] Carte Vitale" stays
//! readable, and ticked by typing an x between the brackets.

use serde::{Deserialize, Serialize};

/// Where an item stands, the weakest first: two versions merge to the stronger.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mark {
    /// `[ ]`: to do.
    Open,
    /// `[-]`: not needed this time.
    Skipped,
    /// `[x]`: done.
    Done,
}

impl Mark {
    fn of(c: char) -> Option<Mark> {
        match c {
            ' ' => Some(Mark::Open),
            'x' | 'X' => Some(Mark::Done),
            '-' => Some(Mark::Skipped),
            _ => None,
        }
    }

    /// The character between the brackets.
    pub fn char(self) -> char {
        match self {
            Mark::Open => ' ',
            Mark::Skipped => '-',
            Mark::Done => 'x',
        }
    }

    /// "open", "skipped", "done".
    pub fn id(self) -> &'static str {
        match self {
            Mark::Open => "open",
            Mark::Skipped => "skipped",
            Mark::Done => "done",
        }
    }

    /// "open", "skipped", "done" (and "not-needed" for skipped); else None.
    pub fn parse(word: &str) -> Option<Mark> {
        match word.trim().to_ascii_lowercase().as_str() {
            "open" | "todo" | "to-do" => Some(Mark::Open),
            "skipped" | "not-needed" | "skip" => Some(Mark::Skipped),
            "done" => Some(Mark::Done),
            _ => None,
        }
    }
}

/// One item of a checklist.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Item {
    /// Its line in the notes, from 0.
    pub line: usize,
    /// Its place among the items, from 1.
    pub number: usize,
    pub mark: Mark,
    pub text: String,
    /// The group it is in ("Bring"); "" for none.
    pub group: String,
    /// The lines indented under it, trimmed.
    pub detail: Vec<String>,
}

/// A line's box, when the line is an item: where its mark is (a byte), the
/// mark, the words after it.
fn boxed(line: &str) -> Option<(usize, Mark, &str)> {
    let indent = line.len() - line.trim_start().len();
    let rest = &line[indent..];
    let bullet = if rest.starts_with(['-', '*', '+']) {
        1
    } else {
        let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
        if digits == 0 || digits > 9 || !rest[digits..].starts_with(['.', ')']) {
            return None;
        }
        digits + 1
    };
    let after = &rest[bullet..];
    let spaces = after.len() - after.trim_start().len();
    if spaces == 0 {
        return None;
    }
    let inner = &after[spaces..];
    let bytes = inner.as_bytes();
    if bytes.len() < 3 || bytes[0] != b'[' || bytes[2] != b']' {
        return None;
    }
    let mark = Mark::of(bytes[1] as char)?;
    let words = &inner[3..];
    // "[x]" alone, or before a space: "[x]word" is no box (GFM).
    if !(words.is_empty() || words.starts_with(char::is_whitespace)) || words.trim().is_empty() {
        return None;
    }
    Some((indent + bullet + spaces + 1, mark, words.trim()))
}

/// Whether a group says what to bring (composed tasks §7.3: the leaving
/// reminder names those items alone): "Bring", "To bring", "Take", « À
/// emporter », « À apporter », or no group (a list of things, said once);
/// never "Check" nor a group of your own, which are not things to take.
pub fn is_bring(group: &str) -> bool {
    let folded: String = crate::text::fold(group.trim()).into_iter().collect::<String>().to_lowercase();
    let folded = folded.trim_end_matches([':', ' ', '\u{a0}', '\u{202f}']).trim();
    matches!(folded, "" | "bring" | "to bring" | "take" | "to take" | "a emporter" | "emporter" | "a apporter" | "apporter" | "a prendre")
}

/// A line that names a group: words ending with a colon ("Bring:",
/// "**Check:**", "## À emporter :"), their name; else None.
fn group_name(line: &str) -> Option<String> {
    let text = line.trim().trim_start_matches('#').trim();
    let text = text.trim_matches(|c| c == '*' || c == '_').trim();
    let name = text.strip_suffix(':')?.trim();
    (!name.is_empty() && name.chars().count() <= 60 && !name.contains(':')).then(|| name.to_string())
}

/// Whether a line opens or closes a fenced block of code.
fn fence(line: &str) -> bool {
    let line = line.trim_start();
    line.starts_with("```") || line.starts_with("~~~")
}

/// The items, and the lines that are the list's (items, their detail, the
/// lines naming their groups), by line.
fn read(notes: &str) -> (Vec<Item>, Vec<bool>) {
    let lines: Vec<&str> = notes.split('\n').map(|l| l.strip_suffix('\r').unwrap_or(l)).collect();
    let mut listed = vec![false; lines.len()];
    let mut items: Vec<Item> = Vec::new();
    let (mut code, mut group) = (false, String::new());
    // A line naming a group, waiting for its items; whether the last words were an item's.
    let mut naming: Option<(usize, String)> = None;
    let mut in_item = false;
    for (i, line) in lines.iter().enumerate() {
        if fence(line) {
            code = !code;
            (naming, group, in_item) = (None, String::new(), false);
            continue;
        }
        if code {
            continue;
        }
        if let Some((_, mark, text)) = boxed(line) {
            if let Some((at, name)) = naming.take() {
                listed[at] = true;
                group = name;
            }
            listed[i] = true;
            items.push(Item { line: i, number: items.len() + 1, mark, text: text.to_string(), group: group.clone(), detail: Vec::new() });
            in_item = true;
            continue;
        }
        if line.trim().is_empty() {
            continue;
        }
        if in_item && line.starts_with([' ', '\t']) {
            if let Some(last) = items.last_mut() {
                last.detail.push(line.trim().to_string());
            }
            listed[i] = true;
            continue;
        }
        // Words that are no item: a group's name waiting for its items, or free text.
        in_item = false;
        group = String::new();
        naming = group_name(line).map(|name| (i, name));
    }
    (items, listed)
}

/// The checklist's items, in order.
pub fn items(notes: &str) -> Vec<Item> {
    read(notes).0
}

/// The notes without their checklist: what is left once its items, their
/// detail and the lines naming their groups are taken out, blank lines kept
/// single.
pub fn free_text(notes: &str) -> String {
    let (_, listed) = read(notes);
    let mut out: Vec<&str> = Vec::new();
    for (i, line) in notes.split('\n').map(|l| l.strip_suffix('\r').unwrap_or(l)).enumerate() {
        if listed.get(i).copied().unwrap_or(false) {
            continue;
        }
        if line.trim().is_empty() && out.last().is_none_or(|l| l.trim().is_empty()) {
            continue;
        }
        out.push(line);
    }
    out.join("\n").trim().to_string()
}

/// The notes with their free text replaced by `free`, their checklist kept
/// as it is, line for line (its marks, bullets and groups): `free` first,
/// then the list. What an agent writes as notes never wipes the list.
pub fn with_free_text(notes: &str, free: &str) -> String {
    let (_, listed) = read(notes);
    let mut list: Vec<&str> = Vec::new();
    for (i, line) in notes.split('\n').map(|l| l.strip_suffix('\r').unwrap_or(l)).enumerate() {
        let kept = listed.get(i).copied().unwrap_or(false);
        if kept {
            list.push(line);
        } else if line.trim().is_empty() && !list.is_empty() && list.last().is_some_and(|l| !l.trim().is_empty()) {
            list.push("");
        }
    }
    let list = list.join("\n").trim().to_string();
    match (free.trim(), list.is_empty()) {
        (free, true) => free.to_string(),
        ("", false) => list,
        (free, false) => format!("{free}\n\n{list}"),
    }
}

/// Why an item could not be ticked: the notes changed since they were read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stale;

/// The notes with the item on `line` set to `mark`: one character changed,
/// every other byte as it was. Stale when that line is no item now.
pub fn ticked(notes: &str, line: usize, mark: Mark) -> Result<String, Stale> {
    let mut start = 0;
    for (i, piece) in notes.split_inclusive('\n').enumerate() {
        if i == line {
            let text = piece.strip_suffix('\n').unwrap_or(piece);
            let text = text.strip_suffix('\r').unwrap_or(text);
            // Its line in the list as `read` sees it: never one in a block of code.
            if !items(notes).iter().any(|item| item.line == line) {
                return Err(Stale);
            }
            let (at, _, _) = boxed(text).ok_or(Stale)?;
            let mut out = String::with_capacity(notes.len());
            out.push_str(&notes[..start + at]);
            out.push(mark.char());
            out.push_str(&notes[start + at + 1..]);
            return Ok(out);
        }
        start += piece.len();
    }
    Err(Stale)
}

/// The notes without the item on `line` and its detail (an item made a
/// step or a task of its own); every other line as it was. Stale when that
/// line is no item now.
pub fn removed(notes: &str, line: usize) -> Result<String, Stale> {
    let (all, _) = read(notes);
    all.iter().find(|i| i.line == line).ok_or(Stale)?;
    let lines: Vec<&str> = notes.split('\n').collect();
    // Its detail, as `read` takes it: the indented lines after it (blank lines
    // between them stay), up to the first other line, never the name of the
    // group that comes next.
    let mut gone = vec![line];
    for (i, l) in lines.iter().enumerate().skip(line + 1) {
        let l = l.strip_suffix('\r').unwrap_or(l);
        if l.trim().is_empty() {
            continue;
        }
        if fence(l) || boxed(l).is_some() || !l.starts_with([' ', '\t']) {
            break;
        }
        gone.push(i);
    }
    let kept: Vec<&str> = lines.iter().enumerate().filter(|(i, _)| !gone.contains(i)).map(|(_, l)| *l).collect();
    Ok(kept.join("\n"))
}

/// The notes with one more item at the end of their list (`text`, its
/// mark, its detail indented under it); a list begun after the notes' words
/// when they have none.
pub fn appended(notes: &str, text: &str, mark: Mark, detail: &[String]) -> String {
    let mut item = format!("- [{}] {}", mark.char(), one_line(text));
    for line in detail.iter().map(|d| one_line(d)).filter(|d| !d.is_empty()) {
        item.push_str(&format!("\n  {line}"));
    }
    let (all, listed) = read(notes);
    let Some(last) = all.last() else {
        let words = notes.trim_end();
        return if words.is_empty() { item } else { format!("{words}\n\n{item}") };
    };
    // After the last item and its detail.
    let lines: Vec<&str> = notes.split('\n').collect();
    let mut end = last.line;
    while end + 1 < lines.len() && listed.get(end + 1).copied().unwrap_or(false) && boxed(lines[end + 1].trim_end_matches('\r')).is_none() {
        end += 1;
    }
    let mut out: Vec<String> = lines[..=end].iter().map(|l| l.to_string()).collect();
    out.push(item);
    out.extend(lines[end + 1..].iter().map(|l| l.to_string()));
    out.join("\n")
}

/// A mark changed by a tap: to do or not needed becomes done; done, to do again.
pub fn toggled(mark: Mark) -> Mark {
    if mark == Mark::Done { Mark::Open } else { Mark::Done }
}

/// How an item is named: its place (1, 2…), or its words, capitals and
/// accents aside. None found, or several with those words: Err, with the
/// items as they are now, so that what is ticked is never a guess.
pub fn find(notes: &str, wanted: &str) -> Result<Item, Vec<Item>> {
    let all = items(notes);
    let wanted = wanted.trim();
    if let Ok(number) = wanted.parse::<usize>() {
        return all.iter().find(|i| i.number == number).cloned().ok_or(all);
    }
    let folded = crate::words::folded(wanted);
    let found: Vec<&Item> = all.iter().filter(|i| crate::words::folded(&i.text) == folded).collect();
    match found.as_slice() {
        [one] => Ok((*one).clone()),
        _ => Err(all),
    }
}

/// One item as `written` writes it: its mark, its words, its detail.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize, Serialize)]
pub struct Entry {
    pub text: String,
    #[serde(default)]
    pub mark: Option<Mark>,
    #[serde(default)]
    pub detail: Vec<String>,
}

/// A group of entries: its name ("Bring"), "" for none.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize, Serialize)]
pub struct Group {
    #[serde(default)]
    pub name: String,
    pub items: Vec<Entry>,
}

/// One line's words: no line break, no control character.
fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ").chars().filter(|c| !c.is_control()).collect()
}

/// Notes in Sioul's form: `free`, a blank line, the items of no group, then
/// each group, its name and a colon on a line of its own, its items one a
/// line ("- [ ] …"), their detail indented. Items of no group come first:
/// after a group they would read as its own. An entry without a mark keeps
/// the one an item of the same words has in `before` (a tick is never
/// lost), else is to do.
pub fn written(free: &str, groups: &[Group], before: &str) -> String {
    let old = items(before);
    let mut out = free.trim().to_string();
    let named = |g: &&Group| !one_line(g.name.trim().trim_end_matches(':')).is_empty();
    let unnamed: Vec<Entry> = groups.iter().filter(|g| !named(g)).flat_map(|g| g.items.clone()).collect();
    let ordered: Vec<Group> = std::iter::once(Group { name: String::new(), items: unnamed }).chain(groups.iter().filter(named).cloned()).collect();
    for group in &ordered {
        let entries: Vec<&Entry> = group.items.iter().filter(|e| !one_line(&e.text).is_empty()).collect();
        if entries.is_empty() {
            continue;
        }
        if !out.is_empty() {
            out.push_str("\n\n");
        }
        let name = one_line(group.name.trim().trim_end_matches(':'));
        if !name.is_empty() {
            out.push_str(&format!("{name}:\n"));
        }
        let lines: Vec<String> = entries
            .iter()
            .map(|entry| {
                let text = one_line(&entry.text);
                let kept = old.iter().find(|i| crate::words::folded(&i.text) == crate::words::folded(&text)).map(|i| i.mark);
                let mark = entry.mark.or(kept).unwrap_or(Mark::Open);
                let mut line = format!("- [{}] {text}", mark.char());
                for detail in entry.detail.iter().map(|d| one_line(d)).filter(|d| !d.is_empty()) {
                    line.push_str(&format!("\n  {detail}"));
                }
                line
            })
            .collect();
        out.push_str(&lines.join("\n"));
    }
    out
}

/// The notes with every mark made a blank: what two versions that differ
/// only by their ticks have alike.
fn unmarked(notes: &str) -> String {
    let marked: Vec<usize> = items(notes).iter().map(|i| i.line).collect();
    notes
        .split('\n')
        .enumerate()
        .map(|(i, line)| match boxed(line.strip_suffix('\r').unwrap_or(line)) {
            Some((at, _, _)) if marked.contains(&i) => format!("{} {}", &line[..at], &line[at + 1..]),
            _ => line.to_string(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Two versions of the same notes that differ only by their marks, merged:
/// `theirs`, each item at the stronger of its two marks. None when they
/// differ otherwise.
pub fn merge_ticks(mine: &str, theirs: &str) -> Option<String> {
    if unmarked(mine).trim_end() != unmarked(theirs).trim_end() {
        return None;
    }
    let (ours, others) = (items(mine), items(theirs));
    if ours.len() != others.len() {
        return None;
    }
    let mut merged = theirs.to_string();
    for (a, b) in ours.iter().zip(&others) {
        if a.mark > b.mark {
            merged = ticked(&merged, b.line, a.mark).ok()?;
        }
    }
    Some(merged)
}

/// Lines rewritten at every change (RFC 5545 §3.8.7): never a difference that matters here.
const STAMPS: &[&str] = &["DTSTAMP", "LAST-MODIFIED", "SEQUENCE"];

/// Each component's lines, in the order the components end, sorted within
/// each: stamps left out, notes unmarked. Two versions alike here hold the
/// same things, but for their ticks.
fn shape(lines: &[String]) -> Vec<Vec<String>> {
    let mut open: Vec<Vec<String>> = Vec::new();
    let mut out: Vec<Vec<String>> = Vec::new();
    for line in lines {
        let name = crate::lines::name(line);
        match name.as_str() {
            "BEGIN" => open.push(vec![line.to_ascii_uppercase()]),
            "END" => {
                if let Some(mut component) = open.pop() {
                    component.sort();
                    out.push(component);
                }
            }
            _ if STAMPS.contains(&name.as_str()) => {}
            "DESCRIPTION" => {
                let value = crate::lines::value(line);
                let head = &line[..line.len() - value.len()];
                if let Some(component) = open.last_mut() {
                    component.push(format!("{head}{}", unmarked(&crate::lines::unescape(value))));
                }
            }
            _ => {
                if let Some(component) = open.last_mut() {
                    component.push(line.clone());
                }
            }
        }
    }
    out
}

/// Two versions of an iCalendar file, yours and the server's, that differ
/// only by the ticks of their notes' checklists and their stamps, merged:
/// the server's, each item at the stronger of its marks (`merge_ticks`).
/// None when they differ otherwise: then yours is set aside as any change
/// made on both sides.
pub fn merge_calendar(mine: &str, theirs: &str) -> Option<String> {
    let (ours, others) = (crate::lines::unfold(mine), crate::lines::unfold(theirs));
    if shape(&ours) != shape(&others) {
        return None;
    }
    let notes = |lines: &[String]| -> Vec<String> { lines.iter().filter(|l| crate::lines::name(l) == "DESCRIPTION").map(|l| crate::lines::unescape(crate::lines::value(l))).collect() };
    let (a, b) = (notes(&ours), notes(&others));
    if a.len() != b.len() {
        return None;
    }
    let merged: Vec<String> = a.iter().zip(&b).map(|(a, b)| merge_ticks(a, b)).collect::<Option<_>>()?;
    let mut k = 0;
    let out: Vec<String> = others
        .iter()
        .map(|line| {
            if crate::lines::name(line) != "DESCRIPTION" {
                return line.clone();
            }
            let value = crate::lines::value(line);
            let head = &line[..line.len() - value.len()];
            let text = &merged[k];
            k += 1;
            if *text == crate::lines::unescape(value) { line.clone() } else { format!("{head}{}", crate::lines::escape(text)) }
        })
        .collect();
    Some(crate::lines::fold(&out))
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOTES: &str = "Take the green folder, it holds the old results.\n\nBring:\n- [ ] Carte Vitale\n- [ ] Mutuelle card\n- [x] Last blood test\n  The paper one, in the green folder.\n- [-] Vaccination record\n\nCheck:\n- [ ] The doctor's address is in the phone";

    #[test]
    fn items_groups_and_detail() {
        let all = items(NOTES);
        let seen: Vec<(usize, usize, Mark, &str, &str)> = all.iter().map(|i| (i.line, i.number, i.mark, i.text.as_str(), i.group.as_str())).collect();
        assert_eq!(
            seen,
            [
                (3, 1, Mark::Open, "Carte Vitale", "Bring"),
                (4, 2, Mark::Open, "Mutuelle card", "Bring"),
                (5, 3, Mark::Done, "Last blood test", "Bring"),
                (7, 4, Mark::Skipped, "Vaccination record", "Bring"),
                (10, 5, Mark::Open, "The doctor's address is in the phone", "Check"),
            ]
        );
        assert_eq!(all[2].detail, ["The paper one, in the green folder."]);
        assert_eq!(free_text(NOTES), "Take the green folder, it holds the old results.");
    }

    #[test]
    fn what_is_no_item() {
        let notes = "* [x] star\n+ [ ] plus\n1. [ ] one\n2) [X] two\n- [x]word\n- [ ]\n-[ ] tight\n- [/] other\n> - [ ] quoted\n```\n- [ ] code\n```\nWords: then words\n- [ ] after words";
        let texts: Vec<String> = items(notes).iter().map(|i| format!("{}{}", i.text, if i.group.is_empty() { String::new() } else { format!(" ({})", i.group) })).collect();
        assert_eq!(texts, ["star", "plus", "one", "two", "after words"], "{notes}");
        // A group's name is a line that ends with a colon, in French too.
        let french = "## À emporter\u{202f}:\n- [ ] Carte Vitale\n\n**Check:**\n\n- [ ] Keys";
        assert_eq!(items(french).iter().map(|i| i.group.as_str()).collect::<Vec<_>>(), ["À emporter", "Check"]);
        assert_eq!(free_text(french), "", "the names of its groups are the list's");
    }

    #[test]
    fn a_tick_changes_one_character() {
        let crlf = NOTES.replace('\n', "\r\n");
        for notes in [NOTES.to_string(), crlf] {
            let line = items(&notes)[0].line;
            let done = ticked(&notes, line, Mark::Done).unwrap();
            assert_eq!(done.len(), notes.len());
            assert_eq!(done.chars().zip(notes.chars()).filter(|(a, b)| a != b).count(), 1);
            assert!(done.contains("- [x] Carte Vitale"));
            assert_eq!(ticked(&done, line, Mark::Skipped).unwrap().matches("[-]").count(), 2);
            assert_eq!(ticked(&done, line, Mark::Open).unwrap(), notes);
        }
        // A line that is no item now: stale, never a guess.
        assert_eq!(ticked(NOTES, 0, Mark::Done), Err(Stale));
        assert_eq!(ticked(NOTES, 99, Mark::Done), Err(Stale));
        assert_eq!(ticked("```\n- [ ] code\n```", 1, Mark::Done), Err(Stale));
        assert_eq!(toggled(Mark::Skipped), Mark::Done);
        assert_eq!(toggled(Mark::Done), Mark::Open);
    }

    #[test]
    fn an_item_is_found_by_its_place_or_its_words() {
        assert_eq!(find(NOTES, "2").unwrap().text, "Mutuelle card");
        assert_eq!(find(NOTES, " carte vitale ").unwrap().number, 1);
        assert_eq!(find("- [ ] Clé USB", "cle usb").unwrap().text, "Clé USB");
        let missing = find(NOTES, "Passport").unwrap_err();
        assert_eq!(missing.len(), 5, "the items as they are now");
        assert!(find(NOTES, "9").is_err());
        assert_eq!(find("- [ ] Keys\n- [x] keys", "Keys").unwrap_err().len(), 2, "two alike: never a guess");
    }

    #[test]
    fn written_in_one_form_its_ticks_kept() {
        let groups = vec![
            Group { name: "Bring:".into(), items: vec![Entry { text: "Carte Vitale".into(), ..Entry::default() }, Entry { text: "last  BLOOD test".into(), detail: vec!["The paper one".into()], ..Entry::default() }] },
            Group { name: String::new(), items: vec![Entry { text: "Keys\nand wallet".into(), mark: Some(Mark::Skipped), ..Entry::default() }] },
            Group { name: "Empty".into(), items: vec![Entry { text: "  ".into(), ..Entry::default() }] },
        ];
        let notes = written("  Free words.  ", &groups, NOTES);
        assert_eq!(notes, "Free words.\n\n- [-] Keys and wallet\n\nBring:\n- [ ] Carte Vitale\n- [x] last BLOOD test\n  The paper one");
        assert_eq!(free_text(&notes), "Free words.");
        assert_eq!(items(&notes).iter().map(|i| i.group.as_str()).collect::<Vec<_>>(), ["", "Bring", "Bring"], "read back in their groups");
        assert_eq!(written("", &[], ""), "");
    }

    #[test]
    fn an_item_taken_out_and_one_added() {
        // The blood test, and its detail, out; nothing else changes.
        let without = removed(NOTES, 5).unwrap();
        assert_eq!(without, NOTES.replace("- [x] Last blood test\n  The paper one, in the green folder.\n", ""));
        // The last item of a group, a blank line and a group after it: that group's name stays, its item in it.
        let last_of_bring = removed(NOTES, 7).unwrap();
        assert_eq!(last_of_bring, NOTES.replace("- [-] Vaccination record\n", ""));
        assert_eq!(items(&last_of_bring).iter().map(|i| (i.text.as_str(), i.group.as_str())).last(), Some(("The doctor's address is in the phone", "Check")));
        // The last item of the last group.
        assert_eq!(removed(NOTES, 10).unwrap(), NOTES.replace("\n- [ ] The doctor's address is in the phone", ""));
        assert_eq!(removed(NOTES, 0), Err(Stale));
        // Back at the end of the list, its detail under it.
        let back = appended(&without, "Last blood test", Mark::Done, &["The paper one, in the green folder.".to_string()]);
        assert!(back.ends_with("- [ ] The doctor's address is in the phone\n- [x] Last blood test\n  The paper one, in the green folder."), "{back}");
        assert_eq!(items(&back).len(), 5);
        // A list begun after the words; or alone.
        assert_eq!(appended("Words.", "Keys", Mark::Open, &[]), "Words.\n\n- [ ] Keys");
        assert_eq!(appended("", "Keys", Mark::Skipped, &[]), "- [-] Keys");
        // In the middle of the notes: after the last item, the words after it kept.
        assert_eq!(appended("- [ ] A\n  detail\n\nAfter.", "B", Mark::Open, &[]), "- [ ] A\n  detail\n- [ ] B\n\nAfter.");
    }

    #[test]
    fn new_free_text_keeps_the_list() {
        let ticked_one = ticked(NOTES, 5, Mark::Open).unwrap();
        let notes = with_free_text(&ticked_one, "Ask about the new treatment.");
        assert_eq!(notes, "Ask about the new treatment.\n\nBring:\n- [ ] Carte Vitale\n- [ ] Mutuelle card\n- [ ] Last blood test\n  The paper one, in the green folder.\n- [-] Vaccination record\n\nCheck:\n- [ ] The doctor's address is in the phone");
        assert_eq!(items(&notes), items(&ticked_one).into_iter().map(|i| Item { line: i.line, ..i }).collect::<Vec<_>>());
        assert_eq!(free_text(&notes), "Ask about the new treatment.");
        assert_eq!(with_free_text(NOTES, ""), NOTES[NOTES.find("Bring:").unwrap()..]);
        assert_eq!(with_free_text("Only words.", "Other words."), "Other words.");
    }

    #[test]
    fn ticks_on_two_devices_merge() {
        let line = |n: usize| items(NOTES)[n].line;
        let mine = ticked(NOTES, line(0), Mark::Done).unwrap();
        let theirs = ticked(&ticked(NOTES, line(1), Mark::Done).unwrap(), line(3), Mark::Open).unwrap();
        let merged = merge_ticks(&mine, &theirs).unwrap();
        let marks: Vec<Mark> = items(&merged).iter().map(|i| i.mark).collect();
        // Every tick kept; "not needed" here, undone there: the stronger, not needed.
        assert_eq!(marks, [Mark::Done, Mark::Done, Mark::Done, Mark::Skipped, Mark::Open]);
        assert_eq!(merge_ticks(&mine, &mine).unwrap(), mine);
        // Words changed on one side: no merge.
        assert!(merge_ticks(&mine, &theirs.replace("Carte Vitale", "Carte vitale")).is_none());
        assert!(merge_ticks(&mine, &format!("{theirs}\n- [ ] One more")).is_none());
    }

    fn todo(notes: &str, extra: &str) -> String {
        let escaped = crate::lines::escape(notes);
        crate::lines::fold(&["BEGIN:VCALENDAR".to_string(), "BEGIN:VTODO".into(), "UID:doctor".into(), "DTSTAMP:20261025T100000Z".into(), "SUMMARY:Going to the doctor".into(), format!("DESCRIPTION:{escaped}"), extra.into(), "END:VTODO".into(), "END:VCALENDAR".into()])
    }

    #[test]
    fn a_calendar_file_merges_when_only_its_ticks_differ() {
        let line = |n: usize| items(NOTES)[n].line;
        let mine = todo(&ticked(NOTES, line(0), Mark::Done).unwrap(), "X-OTHER:1");
        // Theirs: another tick, new stamps, its lines in another order (Nextcloud takes DESCRIPTION out and puts it back).
        let theirs_notes = ticked(NOTES, line(4), Mark::Done).unwrap();
        let theirs = crate::lines::fold(&[
            "BEGIN:VCALENDAR".to_string(),
            "BEGIN:VTODO".into(),
            "UID:doctor".into(),
            "SUMMARY:Going to the doctor".into(),
            "X-OTHER:1".into(),
            "DTSTAMP:20261025T110000Z".into(),
            "SEQUENCE:3".into(),
            format!("DESCRIPTION:{}", crate::lines::escape(&theirs_notes)),
            "END:VTODO".into(),
            "END:VCALENDAR".into(),
        ]);
        let merged = merge_calendar(&mine, &theirs).unwrap();
        let task = crate::tasks::task_of_text(&merged, &jiff::tz::TimeZone::UTC).unwrap();
        let marks: Vec<Mark> = items(&task.notes).iter().map(|i| i.mark).collect();
        assert_eq!(marks, [Mark::Done, Mark::Open, Mark::Done, Mark::Skipped, Mark::Done]);
        assert!(merged.contains("SEQUENCE:3") && merged.contains("DTSTAMP:20261025T110000Z"), "the server's file, its stamps: {merged}");
        assert_eq!(merged.split("\r\n").filter(|l| l.starts_with("DESCRIPTION")).count(), 1);
        // Only the stamps differ: theirs, nothing set aside.
        assert_eq!(merge_calendar(&todo(NOTES, "X-OTHER:1"), &todo(NOTES, "X-OTHER:1").replace("20261025T100000Z", "20261026T090000Z")).unwrap(), todo(NOTES, "X-OTHER:1").replace("20261025T100000Z", "20261026T090000Z"));
        // Anything else changed on one side: no merge.
        assert!(merge_calendar(&mine, &theirs.replace("SUMMARY:Going to the doctor", "SUMMARY:Doctor")).is_none());
        assert!(merge_calendar(&mine, &theirs.replace("X-OTHER:1", "X-OTHER:2")).is_none());
        assert!(merge_calendar(&mine, &theirs.replace("Carte Vitale", "Carte vitale")).is_none());
    }

    /// Checklists made at random, written into a task's file the way Sioul
    /// writes one (escaped, folded at 75 octets, CRLF), read back the same;
    /// each item ticked, only its character changes.
    #[test]
    fn written_into_a_file_and_read_back_the_same() {
        let words = ["Carte Vitale", "Mutuelle, card", "Clé; USB", "Ordonnance d’hier", "Lunettes", "Pièce d'identité\\copie", "Ζ", "パスポート", "a very long item that goes past the seventy five octets of one content line, folded"];
        let mut seed: u64 = 0x5eed;
        let mut next = |n: usize| {
            seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
            (seed >> 33) as usize % n
        };
        let zone = jiff::tz::TimeZone::UTC;
        let now: jiff::Zoned = "2026-10-25T10:00:00+00:00[UTC]".parse().unwrap();
        for _ in 0..200 {
            let groups: Vec<Group> = (0..1 + next(3))
                .map(|g| Group {
                    name: if next(2) == 0 { String::new() } else { ["Bring", "À vérifier", "Check, then"][g % 3].into() },
                    items: (0..1 + next(5)).map(|_| Entry { text: words[next(words.len())].into(), mark: Some([Mark::Open, Mark::Done, Mark::Skipped][next(3)]), detail: if next(4) == 0 { vec![words[next(words.len())].into()] } else { vec![] } }).collect(),
                })
                .collect();
            let notes = written(if next(2) == 0 { "" } else { "Free words, with a comma; and a semicolon." }, &groups, "");
            let edit = crate::tasks::TaskEdit { title: "Doctor".into(), notes: notes.clone(), ..crate::tasks::TaskEdit::default() };
            let text = crate::tasks::new_task(&edit, "doctor", &zone, &now).unwrap();
            assert!(text.split("\r\n").all(|l| l.len() <= 75), "{text}");
            let read = crate::tasks::task_of_text(&text, &zone).unwrap();
            assert_eq!(read.notes, notes);
            assert_eq!(items(&read.notes), items(&notes));
            for item in items(&notes) {
                let mark = toggled(item.mark);
                let changed = ticked(&notes, item.line, mark).unwrap();
                assert_eq!(changed.chars().zip(notes.chars()).filter(|(a, b)| a != b).count(), usize::from(mark.char() != item.mark.char()));
                let after = items(&changed);
                assert_eq!(after.iter().map(|i| if i.line == item.line { mark } else { i.mark }).collect::<Vec<_>>(), after.iter().map(|i| i.mark).collect::<Vec<_>>());
            }
        }
    }
}

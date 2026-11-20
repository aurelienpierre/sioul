// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The Notes page's search inside the notes, their outline, when each was
//! made, and the dated lists (docs/tasks.md, "Notes").

use super::*;
use jiff::Zoned;
use jiff::tz::TimeZone;

fn paris() -> TimeZone {
    TimeZone::get("Europe/Paris").unwrap()
}

/// A note as the index knows it, its text kept beside it for the search.
fn note(path: &str, text: &str, modified: i64) -> (Note, String) {
    let mut n = read(path, text);
    n.kind = kind_of_file(path).unwrap_or_default();
    n.modified = modified;
    (n, text.to_string())
}

fn vault_of(notes: &[(Note, String)]) -> Vault {
    Vault { root: PathBuf::from("/vault"), notes: notes.iter().map(|(n, _)| n.clone()).collect(), folders: Vec::new() }
}

fn found(vault: &Vault, query: &str, kinds: Kinds, texts: &[(Note, String)]) -> Vec<(String, FoundBy, String)> {
    let read = |n: &Note| texts.iter().find(|(m, _)| m.path == n.path).map(|(_, t)| t.clone());
    vault.find(query, kinds, Some(&read)).into_iter().map(|f| (f.note.path.clone(), f.by, f.line)).collect()
}

/// Found by the title, the path or the tags first, then by the text, each
/// newest first; every word, case and accents aside; the line that holds
/// them, as written; nothing read without `texts`.
#[test]
fn notes_found_by_name_then_by_text() {
    let notes = [
        note("admin/Impôts 2026.md", "# Impôts 2026\n\nLe formulaire 2042, avant mai.\n", 100),
        note("home/Soup.md", "# Lentil soup\n\nAdd the cumin, then the bay leaf.\n- [ ] Buy CUMIN\n", 300),
        note("home/Spices.md", "---\ntags: [cumin]\n---\n# Spices\n", 200),
        note("ideas/Fair.md", "# Craft fair\n\nPosters, and a jar of cumin for the stand.\n", 400),
        note("old/Draft.md", "# Draft\n\nNothing here.\n", 500),
    ];
    let vault = vault_of(&notes);
    // "cumin": the tags first, then the texts, the newest first.
    assert_eq!(
        found(&vault, "Cumin", Kinds::All, &notes),
        vec![
            ("home/Spices.md".into(), FoundBy::Name, String::new()),
            ("ideas/Fair.md".into(), FoundBy::Text, "Posters, and a jar of cumin for the stand.".into()),
            ("home/Soup.md".into(), FoundBy::Text, "Add the cumin, then the bay leaf.".into()),
        ]
    );
    // Accents and case aside, in the name and in the text.
    assert_eq!(found(&vault, "IMPOTS", Kinds::All, &notes)[0].0, "admin/Impôts 2026.md");
    assert_eq!(found(&vault, "formulaire", Kinds::All, &notes)[0].2, "Le formulaire 2042, avant mai.");
    // Every word: the name holds one, the text the other; a line that holds them all is shown first.
    assert_eq!(found(&vault, "soup bay", Kinds::All, &notes), vec![("home/Soup.md".into(), FoundBy::Text, "Add the cumin, then the bay leaf.".into())]);
    assert_eq!(found(&vault, "buy cumin", Kinds::All, &notes), vec![("home/Soup.md".into(), FoundBy::Text, "- [ ] Buy CUMIN".into())]);
    assert!(found(&vault, "cumin saffron", Kinds::All, &notes).is_empty(), "every word, or nothing");
    // Names only: the texts are not read.
    let names: Vec<&str> = vault.find("cumin", Kinds::All, None).iter().map(|f| f.note.path.as_str()).collect();
    assert_eq!(names, vec!["home/Spices.md"]);
    // An empty query finds every note, newest first.
    assert_eq!(vault.find("", Kinds::All, None).len(), 5);
    assert_eq!(vault.find("", Kinds::All, None)[0].note.path, "old/Draft.md");
}

/// A note whose text does not read is found by its name alone; one larger
/// than the bound is searched as far as it goes; the line is cut around the
/// word, on one line.
#[test]
fn notes_unreadable_or_too_large_and_the_line_cut() {
    let dir = std::env::temp_dir().join(format!("sioul-notes-find-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("Gone.md"), "# Gone\n\nthe word quillon\n").unwrap();
    let mut large = String::from("# Large\n\nfirst words: quillon\n");
    while large.len() < TEXT_NOTE_BYTES as usize {
        large.push_str("filler filler filler filler filler filler filler filler\n");
    }
    large.push_str("past the bound: zephyrine\n");
    std::fs::write(dir.join("Large.md"), &large).unwrap();
    // Not UTF-8 after its first line: read all the same.
    let mut odd = b"# Odd\n\nquillon ".to_vec();
    odd.extend([0xff, 0xfe, b'\n']);
    std::fs::write(dir.join("Odd.md"), &odd).unwrap();
    let vault = Vault::open(&dir);
    assert_eq!(vault.notes.len(), 2, "a file that is no UTF-8 is not a note at all (Vault::open)");
    std::fs::remove_file(dir.join("Gone.md")).unwrap();
    let read = |n: &Note| search_text(&dir.join(&n.path));
    let paths = |query: &str| vault.find(query, Kinds::All, Some(&read)).iter().map(|f| (f.note.path.clone(), f.by)).collect::<Vec<_>>();
    assert_eq!(paths("quillon"), vec![("Large.md".to_string(), FoundBy::Text)], "a note gone since it was listed is passed over, never asked about");
    assert_eq!(paths("gone"), vec![("Gone.md".to_string(), FoundBy::Name)], "by its name still");
    assert!(paths("zephyrine").is_empty(), "past TEXT_NOTE_BYTES, not read");
    assert_eq!(search_text(&dir.join("Odd.md")).as_deref(), Some("# Odd\n\nquillon \u{fffd}\u{fffd}\n"));
    std::fs::remove_dir_all(&dir).unwrap();

    // The line cut around its word, at a space, said cut; tabs made spaces.
    let long = format!("{}the word quillon here {}", "lead ".repeat(20), "tail ".repeat(20));
    let shown = line_holding(&long, &["quillon"]).unwrap();
    assert!(shown.starts_with('…') && shown.ends_with('…'), "{shown}");
    assert!(shown.contains("the word quillon here"), "{shown}");
    assert!(shown.chars().count() <= LINE_SHOWN + 2, "{shown}");
    assert!(!shown.contains("  "), "cut at spaces: {shown}");
    assert_eq!(line_holding("\t  A\tquillon  ", &["quillon"]).as_deref(), Some("A quillon"));
    // Lowered ASCII at once, or folded one by one: the same.
    assert_eq!(folded("Quillon, ÉTÉ 2026"), crate::text::fold("Quillon, ÉTÉ 2026").into_iter().collect::<String>());
    assert_eq!(folded("Quillon 2026\t!"), "quillon 2026\t!");
    // An accented word found by its folded form, the line shown as written.
    let accented = format!("{}Élodie", "é".repeat(100));
    assert!(line_holding(&accented, &["elodie"]).unwrap().ends_with("Élodie"));
}

/// The Type keeps what it says, in the search as in every list; PDFs and
/// sounds are found by their names only.
#[test]
fn notes_of_one_type() {
    let notes = [
        note("memos/Market day.ogg", "", 50),
        note("scans/Market permit.pdf", "", 40),
        note("scans/Market stall.png", "", 30),
        note("Market prices.ods", "", 20),
        note("Market.md", "# Notes\n\nmarket at dawn\n", 10),
    ];
    let vault = vault_of(&notes);
    let kinds = |k: Kinds| found(&vault, "market", k, &notes).into_iter().map(|(p, _, _)| p).collect::<Vec<_>>();
    assert_eq!(kinds(Kinds::All).len(), 5);
    assert_eq!(kinds(Kinds::Sound), vec!["memos/Market day.ogg"]);
    assert_eq!(kinds(Kinds::Pdf), vec!["scans/Market permit.pdf"]);
    assert_eq!(kinds(Kinds::Text), vec!["Market.md"]);
    assert_eq!(kinds(Kinds::Other), vec!["scans/Market stall.png", "Market prices.ods"]);
    assert_eq!(found(&vault, "dawn", Kinds::Sound, &notes), vec![], "a sound is never searched inside");
    assert!(vault.names_only(Kinds::All) && vault.names_only(Kinds::Pdf) && !vault.names_only(Kinds::Text) && !vault.names_only(Kinds::Other));
    assert!(!vault_of(&notes[2..]).names_only(Kinds::All), "no PDF nor sound: nothing to say");
    assert_eq!((Kinds::named("sound"), Kinds::named("anything"), Kinds::Pdf.name()), (Kinds::Sound, Kinds::All, "pdf"));
}

/// Headings by level, as the note is drawn: the two underlined forms, never
/// in code, a quote, a list or the front matter; an empty one left out;
/// their words as shown.
#[test]
fn notes_outline() {
    let text = "---\ntitle: Plan\nnotes: |\n# not a heading\n---\n\
                # The plan\n\nIntro.\n\n## Money **first**\n\n```\n# code, not a heading\n```\n\n    # indented code\n\n\
                > # quoted\n\n- # in a list\n\n### [[Overview|The overview]] and `code`\n\n#\n\n##   \n\n\
                Second part\n===========\n\nA third\n---\n\n#no space is a tag\n\n###### Six\n\n####### Seven is text\n";
    let headings = outline(text);
    let shape: Vec<(u8, &str, usize)> = headings.iter().map(|h| (h.level, h.text.as_str(), h.line)).collect();
    assert_eq!(shape, vec![(1, "The plan", 6), (2, "Money first", 10), (3, "[[Overview|The overview]] and code", 22), (1, "Second part", 28), (2, "A third", 31), (6, "Six", 36)]);
    // A link's words alone, once the window made it a link (`Vault::with_links`).
    assert_eq!(outline("## See [the overview](<sioul:note/Overview.md>)\n")[0].text, "See the overview");
    assert_eq!(outline("# One\n").len(), 1);
    assert!(outline("").is_empty() && outline("Just words.\n").is_empty());
    // A note without front matter whose first line is a rule: counted from the top.
    assert_eq!(outline("---\n\n# After a rule\n").iter().map(|h| h.line).collect::<Vec<_>>(), vec![3]);
}

/// When a note was made: its front matter's `created`, else `date`, in
/// each form the files write dates; else not known (the file's day comes in
/// `Vault::open`).
#[test]
fn notes_made_by_their_front_matter() {
    let zone = paris();
    let at = |text: &str| front_made(&front_matter(text).0, &zone);
    let paris_midnight = |y: i16, m: i8, d: i8| jiff::civil::date(y, m, d).to_zoned(zone.clone()).unwrap().timestamp().as_second();
    assert_eq!(at("---\ncreated: 2026-10-05\n---\n"), Some(paris_midnight(2026, 10, 5)));
    assert_eq!(at("---\ncreated: \"2026-10-05\"\n---\n"), Some(paris_midnight(2026, 10, 5)), "quoted");
    assert_eq!(at("---\ncreated: 2026-10-05T09:12:00+02:00\n---\n"), Some(paris_midnight(2026, 10, 5) + 9 * 3600 + 12 * 60));
    assert_eq!(at("---\ncreated: 2026-10-05T07:12:00Z\n---\n"), Some(paris_midnight(2026, 10, 5) + 9 * 3600 + 12 * 60));
    assert_eq!(at("---\ncreated: 2026-10-05T09:12\n---\n"), Some(paris_midnight(2026, 10, 5) + 9 * 3600 + 12 * 60), "a clock's time, in the zone");
    assert_eq!(at("---\ncreated: 2026-10-05 09:12\n---\n"), Some(paris_midnight(2026, 10, 5) + 9 * 3600 + 12 * 60), "with a space");
    assert_eq!(at("---\ndate: 2025-03-01\n---\n"), Some(paris_midnight(2025, 3, 1)), "date, when there is no created");
    assert_eq!(at("---\ncreated: last spring\ndate: 2025-03-01\n---\n"), Some(paris_midnight(2025, 3, 1)), "created that is no date: date");
    assert_eq!(at("---\ncreated: 5 October 2026\n---\n"), None, "never guessed");
    assert_eq!(read("n.md", "# No front matter\n").created, None);
    assert!(read("n.md", "---\ndate: 2025-03-01\n---\n").created.is_some(), "read() says it too");
    assert_eq!(front_moment("2026-02-30", &zone), None, "no such day");
}

/// The file's own day when the front matter says none, never its last
/// change; Sioul writing a note again keeps the day its file was first made.
#[test]
fn notes_made_by_their_files() {
    let dir = std::env::temp_dir().join(format!("sioul-notes-born-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("a")).unwrap();
    std::fs::write(dir.join("a/Plain.md"), "# Plain\n").unwrap();
    std::fs::write(dir.join("Dated.md"), "---\ncreated: 2020-01-02\n---\n# Dated\n").unwrap();
    let first = std::fs::metadata(dir.join("a/Plain.md")).ok().as_ref().and_then(born);
    let vault = Vault::open(&dir);
    assert_eq!(vault.note("a/Plain.md").unwrap().created, first, "the system's day, or none where it does not know it");
    assert_eq!(vault.note("Dated.md").unwrap().created, front_moment("2020-01-02", &crate::zone::current()), "the front matter first");
    let Some(first) = first else { return std::fs::remove_dir_all(&dir).unwrap() };
    // A second later, Sioul writes it again: a new file takes its place; its first day stays.
    std::thread::sleep(std::time::Duration::from_millis(1100));
    write(&dir, "a/Plain.md", "# Plain\n\nMore.\n").unwrap();
    let again = std::fs::metadata(dir.join("a/Plain.md")).ok().as_ref().and_then(born).unwrap();
    assert!(again > first, "the system dates the new file now");
    let vault = Vault::open(&dir);
    let plain = vault.note("a/Plain.md").unwrap();
    assert_eq!(plain.created, Some(first), "made when it was first made");
    assert!(plain.modified >= again);
    // Renamed, then its folder renamed, then to the trash and back: its day follows.
    let (path, _) = rename(&vault, "a/Plain.md", "Still plain").unwrap();
    let (_, _) = rename_folder(&Vault::open(&dir), "a", "b").unwrap();
    let moved = path.replacen("a/", "b/", 1);
    let trashed = trash(&dir, &moved).unwrap();
    untrash(&dir, &trashed, &moved).unwrap();
    write(&dir, &moved, "# Plain\n\nAgain.\n").unwrap();
    assert_eq!(Vault::open(&dir).note(&moved).unwrap().created, Some(first));
    std::fs::remove_dir_all(&dir).unwrap();
}

/// The dated lists' groups at the edges of a day, a week, a month and a
/// year, in the zone of "now"; newest first; not known last.
#[test]
fn notes_dated_groups_at_their_edges() {
    let zone = paris();
    let local = |y: i16, m: i8, d: i8, h: i8, min: i8| jiff::civil::date(y, m, d).at(h, min, 0, 0).to_zoned(zone.clone()).unwrap();
    let s = |z: Zoned| Some(z.timestamp().as_second());
    // Wednesday 18 November 2026, 10:00 in Paris: the week began on Monday 16.
    let now = local(2026, 11, 18, 10, 0);
    let cases = [
        (s(local(2026, 11, 19, 0, 0)), When::After),
        (s(local(2026, 11, 18, 0, 0)), When::Today),
        (s(local(2026, 11, 18, 23, 59)), When::Today),
        // 23:30 in London's winter is 00:30 in Paris: today, here.
        (Some(jiff::civil::date(2026, 11, 17).at(23, 30, 0, 0).to_zoned(TimeZone::UTC).unwrap().timestamp().as_second()), When::Today),
        (s(local(2026, 11, 17, 23, 59)), When::Week),
        (s(local(2026, 11, 16, 0, 0)), When::Week),
        (s(local(2026, 11, 15, 23, 59)), When::Month),
        (s(local(2026, 11, 1, 0, 0)), When::Month),
        (s(local(2026, 10, 31, 23, 59)), When::Year),
        (s(local(2026, 1, 1, 0, 0)), When::Year),
        (s(local(2025, 12, 31, 23, 59)), When::In(2025)),
        (s(local(2019, 6, 1, 12, 0)), When::In(2019)),
        (None, When::Unknown),
        (Some(0), When::Unknown),
    ];
    for (at, group) in cases {
        assert_eq!(when(at, &now), group, "{at:?}");
    }
    // A week that began last month: its days are this week, the days before it not this month.
    let december = local(2026, 12, 2, 9, 0);
    assert_eq!(when(s(local(2026, 11, 30, 8, 0)), &december), When::Week);
    assert_eq!(when(s(local(2026, 11, 29, 8, 0)), &december), When::Year);
    // A week that began last year: this week still; the day before it, its year.
    let new_year = local(2027, 1, 1, 9, 0);
    assert_eq!(when(s(local(2026, 12, 28, 8, 0)), &new_year), When::Week);
    assert_eq!(when(s(local(2026, 12, 27, 8, 0)), &new_year), When::In(2026));
    assert_eq!(When::In(2025).key(), "2025");

    // The lists: newest first under their groups, not known last; by last change, or by when made.
    let at = |y, m, d| local(y, m, d, 12, 0).timestamp().as_second();
    let mk = |path: &str, modified: i64, created: Option<i64>| Note { path: path.into(), modified, created, ..Note::default() };
    let notes = [
        mk("a.md", at(2026, 11, 18), None),
        mk("b.md", at(2026, 11, 2), Some(at(2024, 5, 1))),
        mk("c.md", at(2025, 2, 1), Some(at(2026, 11, 17))),
        mk("d.md", at(2026, 11, 3), Some(at(2024, 6, 1))),
        mk("e.md", at(2023, 4, 1), Some(at(2025, 1, 9))),
    ];
    let shape = |by: Dated| dated(&notes, by, &now).into_iter().map(|(w, ns)| (w.key(), ns.iter().map(|n| n.path.as_str()).collect::<Vec<_>>())).collect::<Vec<_>>();
    assert_eq!(shape(Dated::Changed), vec![("today".into(), vec!["a.md"]), ("month".into(), vec!["d.md", "b.md"]), ("2025".into(), vec!["c.md"]), ("2023".into(), vec!["e.md"])], "the latest year first");
    assert_eq!(shape(Dated::Created), vec![("week".into(), vec!["c.md"]), ("2025".into(), vec!["e.md"]), ("2024".into(), vec!["d.md", "b.md"]), ("unknown".into(), vec!["a.md"])]);
}

/// How long the search takes on a made-up vault of a few thousand notes,
/// texts read again at each search or kept: run by hand,
/// `cargo test --release -p sioul-core notes_search_measure -- --ignored --nocapture`
/// (SIOUL_NOTES_MEASURE names the folder to write it in; the system's
/// temporary folder by default).
#[test]
#[ignore]
fn notes_search_measure() {
    let base = std::env::var_os("SIOUL_NOTES_MEASURE").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    let dir = base.join(format!("sioul-notes-measure-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let words = ["lantern", "orchard", "meadow", "quill", "harbour", "ember", "thistle", "copper", "willow", "saffron", "granite", "fennel", "marble", "cobalt", "juniper", "pebble"];
    let (count, mut bytes) = (3000usize, 0usize);
    let mut seed = 7u64;
    let mut next = || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (seed >> 33) as usize
    };
    for i in 0..count {
        let folder = format!("folder {}/sub {}", i % 40, i % 7);
        std::fs::create_dir_all(dir.join(&folder)).unwrap();
        // Most notes short, some long: 0.5 to 40 KB.
        let lines = 10 + next() % if i % 20 == 0 { 600 } else { 60 };
        let mut text = format!("---\ntags: [{}]\ncreated: 2025-0{}-1{}\n---\n# Note {i}\n\n", words[i % words.len()], 1 + i % 9, i % 10);
        for l in 0..lines {
            if l % 12 == 0 {
                text.push_str(&format!("## Part {l}\n"));
            }
            for _ in 0..10 {
                text.push_str(words[next() % words.len()]);
                text.push(' ');
            }
            text.push('\n');
        }
        // Half the notes hold accented words, as French notes do: they take the slower folding.
        if i % 2 == 0 {
            text.push_str("Le café de la forêt, été comme hiver.\n");
        }
        if i == count - 1 {
            text.push_str("the one zanzibar line\n");
        }
        bytes += text.len();
        std::fs::write(dir.join(format!("{folder}/note {i}.md")), text).unwrap();
    }
    let time = |what: &str, f: &mut dyn FnMut() -> usize| {
        let start = std::time::Instant::now();
        let n = f();
        println!("{what}: {:.1} ms ({n})", start.elapsed().as_secs_f64() * 1000.0);
    };
    println!("{count} notes, {:.1} MB of text", bytes as f64 / 1e6);
    let mut vault = Vault::default();
    time("Vault::open (the load the window already makes at each change)", &mut || {
        vault = Vault::open(&dir);
        vault.notes.len()
    });
    time("names only", &mut || vault.find("zanzibar", Kinds::All, None).len());
    let read = |n: &Note| search_text(&dir.join(&n.path));
    time("texts read again, a word in one note", &mut || vault.find("zanzibar", Kinds::All, Some(&read)).len());
    time("texts read again, a word in every note", &mut || vault.find("lantern", Kinds::All, Some(&read)).len());
    time("texts read again, two words", &mut || vault.find("copper willow", Kinds::All, Some(&read)).len());
    let kept: std::collections::HashMap<String, String> = vault.notes.iter().map(|n| (n.path.clone(), search_text(&dir.join(&n.path)).unwrap_or_default())).collect();
    println!("texts kept: {:.1} MB held", kept.values().map(String::capacity).sum::<usize>() as f64 / 1e6);
    let keep = |n: &Note| kept.get(&n.path).cloned();
    time("texts kept, a word in one note", &mut || vault.find("zanzibar", Kinds::All, Some(&keep)).len());
    time("texts kept, a word in every note", &mut || vault.find("lantern", Kinds::All, Some(&keep)).len());
    let longest = vault.notes.iter().filter_map(|n| search_text(&dir.join(&n.path))).max_by_key(String::len).unwrap_or_default();
    time(&format!("outline of the longest note ({} KB)", longest.len() / 1000), &mut || outline(&longest).len());
    time("dated by when made", &mut || dated(&vault.notes, Dated::Created, &crate::zone::now()).len());
    std::fs::remove_dir_all(&dir).unwrap();
}

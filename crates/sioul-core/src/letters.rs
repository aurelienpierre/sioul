// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Paper letters (docs/porch.md, "Paper letters"): a scan, a phone photo or a
//! PDF dropped in a folder (by you, or a helper who opens the post), read by
//! OCR, then understood by rules: who sent it, what it is, the amount, and the
//! date by which something is asked, as a date, legal delays included ("dans
//! un délai de deux mois à compter de la notification"). It waits outside like
//! mail, and comes as a card in the Porch's window, with a task for its date
//! and its scan filed. The envelope stays outside; the date is computed once.
//!
//! The not-knowing costs more than the work; a date written "within 30 days"
//! is remembered by no one (time-based remembering fails, Landsiedel et al.
//! 2017); clearer material and the amount shown raise what people claim
//! (Bhargava & Manoli 2015).
//!
//! `letters/letters.toml` in the case store, each letter's text beside it
//! (`letters/<id>.txt`), the scans filed in `letters/<year>/`.

use crate::money::Money;
use jiff::ToSpan;
use jiff::civil::Date;
use serde::Serialize;
use std::path::{Path, PathBuf};

pub const FOLDER: &str = "letters";
pub const MANIFEST: &str = "letters.toml";

/// What a letter is.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// "Mise en demeure": a last formal step before others.
    FormalNotice,
    /// A decision, with its ways of appeal.
    Decision,
    TaxNotice,
    Reminder,
    Bill,
    Appointment,
    Acknowledgment,
    Attestation,
    Contract,
    #[default]
    Other,
}

impl Kind {
    pub fn id(self) -> &'static str {
        match self {
            Kind::FormalNotice => "formal-notice",
            Kind::Decision => "decision",
            Kind::TaxNotice => "tax-notice",
            Kind::Reminder => "reminder",
            Kind::Bill => "bill",
            Kind::Appointment => "appointment",
            Kind::Acknowledgment => "acknowledgment",
            Kind::Attestation => "attestation",
            Kind::Contract => "contract",
            Kind::Other => "other",
        }
    }

    pub fn of(id: &str) -> Kind {
        [Kind::FormalNotice, Kind::Decision, Kind::TaxNotice, Kind::Reminder, Kind::Bill, Kind::Appointment, Kind::Acknowledgment, Kind::Attestation, Kind::Contract].into_iter().find(|k| k.id() == id).unwrap_or_default()
    }
}

/// Folded words, punctuation as spaces, padded: " avis d impot 2026 ".
fn plain(text: &str) -> String {
    let words: String = crate::text::fold(text).into_iter().map(|c| if c.is_alphanumeric() { c } else { ' ' }).collect();
    format!(" {} ", words.split_whitespace().collect::<Vec<_>>().join(" "))
}

fn has(plain: &str, phrases: &[&str]) -> bool {
    phrases.iter().any(|p| plain.contains(&format!(" {p} ")))
}

/// Bodies that write to everyone, by the words their letters carry.
const SENDERS: &[(&[&str], &str)] = &[
    (&["caisse d allocations familiales", "caf"], "CAF"),
    (&["assurance maladie", "cpam", "caisse primaire d assurance maladie"], "Assurance Maladie"),
    (&["finances publiques", "dgfip", "centre des finances publiques", "service des impots", "tresor public"], "Finances publiques"),
    (&["urssaf"], "Urssaf"),
    (&["france travail", "pole emploi"], "France Travail"),
    (&["maison departementale des personnes handicapees", "mdph"], "MDPH"),
    (&["prefecture", "prefet"], "Préfecture"),
    (&["tribunal judiciaire", "tribunal administratif", "tribunal de proximite", "tribunal"], "Tribunal"),
    (&["commissaire de justice", "huissier de justice", "huissier"], "Commissaire de justice"),
    (&["centre hospitalier", "hopital", "chu", "clinique"], "Hôpital"),
    (&["la banque postale"], "La Banque Postale"),
    (&["mairie", "hotel de ville"], "Mairie"),
    (&["edf", "electricite de france"], "EDF"),
    (&["engie"], "Engie"),
    (&["agirc arrco", "carsat", "assurance retraite"], "Assurance retraite"),
];

/// What a letter says, by rules.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Reading {
    pub sender: String,
    pub kind: Kind,
    pub amount: Option<Money>,
    /// The letter's own date ("Paris, le 3 octobre 2026").
    pub dated: Option<Date>,
    /// The date by which something is asked, and the words that set it.
    pub deadline: Option<Date>,
    pub why: String,
    /// An appointment: its day, and its time when written.
    pub appointment: Option<(Date, Option<(i8, i8)>)>,
    /// Sent registered, with acknowledgment of receipt.
    pub registered: bool,
    /// Your number with them: "Numéro allocataire : 1234567".
    pub reference: String,
}

const MONTHS: &[(&str, i8)] = &[
    ("janvier", 1), ("fevrier", 2), ("mars", 3), ("avril", 4), ("mai", 5), ("juin", 6), ("juillet", 7), ("aout", 8), ("septembre", 9), ("octobre", 10), ("novembre", 11), ("decembre", 12),
    ("janv", 1), ("fevr", 2), ("fev", 2), ("avr", 4), ("juil", 7), ("sept", 9), ("oct", 10), ("nov", 11), ("dec", 12),
    ("january", 1), ("february", 2), ("march", 3), ("april", 4), ("may", 5), ("june", 6), ("july", 7), ("august", 8), ("september", 9), ("october", 10), ("november", 11), ("december", 12),
    ("jan", 1), ("feb", 2), ("mar", 3), ("apr", 4), ("jun", 6), ("jul", 7), ("aug", 8), ("sep", 9),
];

/// The dates written in a plain text, with where each starts (in words):
/// "15 novembre 2026", "1er octobre 2026", "15/11/2026", "15.11.26", "November 15, 2026".
fn dates(words: &[&str]) -> Vec<(usize, Date)> {
    let mut out = Vec::new();
    let month = |w: &str| MONTHS.iter().find(|(name, _)| *name == w).map(|(_, m)| *m);
    let year = |w: &str| w.parse::<i16>().ok().filter(|y| (1990..=2100).contains(y));
    let day = |w: &str| w.trim_end_matches("er").trim_end_matches("st").trim_end_matches("nd").trim_end_matches("rd").trim_end_matches("th").parse::<i8>().ok().filter(|d| (1..=31).contains(d));
    let mut i = 0;
    while i < words.len() {
        // 15 novembre 2026
        if let (Some(d), Some(m), Some(y)) = (day(words[i]), words.get(i + 1).and_then(|w| month(w)), words.get(i + 2).and_then(|w| year(w)))
            && let Ok(date) = Date::new(y, m, d)
        {
            out.push((i, date));
            i += 3;
            continue;
        }
        // November 15 2026
        if let (Some(m), Some(d), Some(y)) = (month(words[i]), words.get(i + 1).and_then(|w| day(w)), words.get(i + 2).and_then(|w| year(w)))
            && let Ok(date) = Date::new(y, m, d)
        {
            out.push((i, date));
            i += 3;
            continue;
        }
        // 15 11 2026 (from 15/11/2026, once punctuation is a space)
        if let (Some(d), Some(m), Some(y)) = (day(words[i]), words.get(i + 1).and_then(|w| w.parse::<i8>().ok()).filter(|m| (1..=12).contains(m)), words.get(i + 2))
            && words[i].len() <= 2
            && let Some(y) = year(y).or_else(|| y.parse::<i16>().ok().filter(|y| *y < 100 && y.to_string().len() <= 2).map(|y| 2000 + y))
            && let Ok(date) = Date::new(y, m, d)
        {
            out.push((i, date));
            i += 3;
            continue;
        }
        i += 1;
    }
    out
}

fn number_word(w: &str) -> Option<i64> {
    match w {
        "un" | "une" | "one" => Some(1),
        "deux" | "two" => Some(2),
        "trois" | "three" => Some(3),
        "quatre" | "four" => Some(4),
        "cinq" | "five" => Some(5),
        "six" => Some(6),
        "huit" | "eight" => Some(8),
        "dix" | "ten" => Some(10),
        "quinze" | "fifteen" => Some(15),
        "trente" | "thirty" => Some(30),
        _ => w.parse().ok().filter(|n: &i64| *n > 0 && *n <= 366),
    }
}

/// Reads a letter's text. `received` is the day it came (its scan's day):
/// a delay counted from the notification counts from there.
pub fn read(text: &str, received: Date) -> Reading {
    let mut reading = Reading::default();
    let all = plain(text);
    let words: Vec<&str> = all.split_whitespace().collect();
    // Who: a known body in the first lines, else the first line of the letterhead.
    let head: String = text.lines().map(str::trim).filter(|l| !l.is_empty()).take(25).collect::<Vec<_>>().join("\n");
    let head_plain = plain(&head);
    reading.sender = SENDERS.iter().find(|(words, _)| has(&head_plain, words)).or_else(|| SENDERS.iter().find(|(words, _)| has(&all, words))).map(|(_, name)| name.to_string()).unwrap_or_else(|| text.lines().map(str::trim).find(|l| l.chars().filter(|c| c.is_alphabetic()).count() >= 3).unwrap_or("").chars().take(60).collect());
    // What it is, the gravest first.
    reading.kind = if has(&all, &["mise en demeure"]) {
        Kind::FormalNotice
    } else if has(&all, &["avis d impot", "avis d imposition", "impot sur le revenu", "taxe fonciere", "taxe d habitation"]) {
        Kind::TaxNotice
    } else if has(&all, &["voies et delais de recours", "delais et voies de recours", "voies de recours", "decision"]) && has(&all, &["recours", "contester"]) {
        Kind::Decision
    } else if has(&all, &["relance", "rappel", "dernier avis", "second avis", "reminder"]) {
        Kind::Reminder
    } else if has(&all, &["convocation", "vous etes convoque", "vous etes convoquee", "rendez vous", "appointment"]) {
        Kind::Appointment
    } else if has(&all, &["facture", "avis des sommes a payer", "titre executoire", "montant a payer", "somme a payer", "invoice"]) {
        Kind::Bill
    } else if has(&all, &["accuse de reception", "nous avons bien recu", "we have received"]) {
        Kind::Acknowledgment
    } else if has(&all, &["attestation", "certificat"]) {
        Kind::Attestation
    } else if has(&all, &["contrat", "avenant", "conditions generales"]) {
        Kind::Contract
    } else {
        Kind::Other
    };
    reading.registered = has(&all, &["recommande", "lettre recommandee", "lrar"]) && has(&all, &["accuse de reception", "avec ar", "lrar", "avis de reception"]);
    // The amount: on a line that asks for it, else the first one after a word that names one.
    let asks = ["a payer", "a regler", "reste du", "montant du", "somme due", "montant total", "total a payer", "somme de", "montant de", "amount due", "total due"];
    reading.amount = text
        .lines()
        .find(|line| has(&plain(line), &asks))
        .and_then(|line| crate::money::find_amount(line))
        .or_else(|| crate::money::find_amount(text))
        .map(|a| Money(a.money.cents().abs()));
    let found = dates(&words);
    // Its own date: the first date in the head, at its start ("Paris, le 3 octobre 2026").
    let head_words = head_plain.split_whitespace().count();
    reading.dated = found.iter().find(|(at, _)| *at < head_words.min(80)).map(|(_, d)| *d);
    // The deadline: a date after a word that asks by when.
    let before = [&["avant", "le"][..], &["au", "plus", "tard", "le"], &["date", "limite"], &["echeance"], &["jusqu", "au"], &["payable", "avant"], &["no", "later", "than"], &["due", "date"], &["due", "by"], &["by"], &["before"]];
    'dates: for (at, date) in &found {
        for words_before in before {
            let n = words_before.len();
            // The asking words within the six words before the date.
            let start = at.saturating_sub(6 + n);
            for k in start..*at {
                if k + n <= words.len() && words[k..k + n] == *words_before && date >= &received.checked_sub(30.days()).unwrap_or(received) {
                    reading.deadline = Some(*date);
                    reading.why = words[k..(*at + 3).min(words.len())].join(" ");
                    break 'dates;
                }
            }
        }
    }
    // A delay: "dans un délai de deux mois à compter de la notification", "sous huitaine".
    if reading.deadline.is_none() {
        for (i, w) in words.iter().enumerate() {
            let span = match *w {
                "delai" | "within" if i + 3 < words.len() => {
                    let (n, unit) = if words[i + 1] == "de" { (number_word(words[i + 2]), words.get(i + 3)) } else { (number_word(words[i + 1]), words.get(i + 2)) };
                    match (n, unit.copied()) {
                        (Some(n), Some("mois" | "months" | "month")) => Some(n.months()),
                        (Some(n), Some("jours" | "days" | "day" | "jour")) => Some(n.days()),
                        (Some(n), Some("semaines" | "weeks" | "semaine" | "week")) => Some((n * 7).days()),
                        _ => None,
                    }
                }
                "sous" if words.get(i + 1) == Some(&"huitaine") => Some(8.days()),
                "sous" if words.get(i + 1) == Some(&"quinzaine") => Some(15.days()),
                _ => None,
            };
            let Some(span) = span else { continue };
            // From the notification or the reception: the day it came; from the letter: its date.
            let rest = words[i..(i + 14).min(words.len())].join(" ");
            let from_letter = rest.contains("date du present") || rest.contains("date de la presente") || rest.contains("date of this");
            let base = if from_letter { reading.dated.unwrap_or(received) } else { received };
            if let Ok(date) = base.checked_add(span) {
                reading.deadline = Some(date);
                reading.why = words[i.saturating_sub(2)..(i + 10).min(words.len())].join(" ");
                break;
            }
        }
    }
    // An appointment: its day, and "à 10 h 30" / "at 10:30" after it.
    if reading.kind == Kind::Appointment
        && let Some((at, date)) = found.iter().find(|(_, d)| *d >= received)
    {
        let after: Vec<&str> = words[(*at + 3).min(words.len())..(*at + 9).min(words.len())].to_vec();
        let time = after.iter().position(|w| *w == "a" || *w == "at").and_then(|k| {
            let hour = after.get(k + 1)?.trim_end_matches('h').parse::<i8>().ok().filter(|h| (0..24).contains(h))?;
            let minute = after.get(k + 2).filter(|w| **w == "h").and_then(|_| after.get(k + 3)).or_else(|| after.get(k + 2)).and_then(|m| m.parse::<i8>().ok()).filter(|m| (0..60).contains(m)).unwrap_or(0);
            Some((hour, minute))
        });
        reading.appointment = Some((*date, time));
    }
    // The words that set the date, as the letter writes them (accents and all).
    if !reading.why.is_empty() {
        reading.why = as_written(text, &reading.why);
    }
    // Your number with them, as written on its line: "Référence : 2026-ASAP-01234".
    let labels = ["numero allocataire", "numero fiscal", "numero de dossier", "n dossier", "reference", "ref", "numero de securite sociale", "numero client", "account number"];
    'lines: for line in text.lines() {
        let folded = plain(line);
        if !labels.iter().any(|l| folded.contains(&format!(" {l} "))) {
            continue;
        }
        let after = line.split_once(':').map_or(line, |(_, rest)| rest);
        for token in after.split_whitespace() {
            let token = token.trim_matches(|c: char| !c.is_alphanumeric());
            if token.chars().filter(char::is_ascii_digit).count() >= 4 {
                reading.reference = token.to_string();
                break 'lines;
            }
        }
    }
    reading
}

/// The words of a folded phrase as `text` writes them: "delai de deux mois" → "délai de deux mois".
fn as_written(text: &str, phrase: &str) -> String {
    let flat: Vec<char> = text.split_whitespace().collect::<Vec<_>>().join(" ").chars().collect();
    // Folded one character for one, so positions hold.
    let folded: Vec<char> = flat.iter().map(|&c| crate::text::fold_char(c)).map(|c| if c.is_alphanumeric() { c } else { ' ' }).collect();
    let mut spans: Vec<(usize, usize)> = Vec::new();
    let mut start = None;
    for (i, c) in folded.iter().enumerate() {
        match (c.is_alphanumeric(), start) {
            (true, None) => start = Some(i),
            (false, Some(s)) => {
                spans.push((s, i));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = start {
        spans.push((s, folded.len()));
    }
    let words: Vec<String> = spans.iter().map(|(a, b)| folded[*a..*b].iter().collect()).collect();
    let wanted: Vec<&str> = phrase.split_whitespace().collect();
    let n = wanted.len();
    (0..words.len().saturating_sub(n.saturating_sub(1)))
        .find(|&k| words[k..k + n].iter().map(String::as_str).eq(wanted.iter().copied()))
        .map(|k| flat[spans[k].0..spans[k + n - 1].1].iter().collect())
        .unwrap_or_else(|| phrase.to_string())
}

/// One letter kept.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Letter {
    pub id: String,
    /// Its scan: from the case store, or anywhere.
    pub file: String,
    /// The scan as it was when read: its name, size and time, to read it once.
    pub source: String,
    pub received: Option<Date>,
    pub reading: Reading,
    /// The project it belongs with.
    pub case: String,
    /// "new" until you are done with it; "done" then.
    pub status: String,
    /// The task made for its date.
    pub task: String,
    /// Why it could not be read ("no OCR program"), if it could not.
    pub problem: String,
}

/// The letters of a case store.
#[derive(Debug, Clone, Default)]
pub struct Letters {
    pub root: PathBuf,
    pub list: Vec<Letter>,
}

impl Letters {
    pub fn folder(root: &Path) -> PathBuf {
        root.join(FOLDER)
    }

    pub fn load(root: &Path) -> Result<Letters, String> {
        let path = Letters::folder(root).join(MANIFEST);
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Letters { root: root.to_path_buf(), list: Vec::new() }),
            Err(e) => return Err(format!("{}: {e}", path.display())),
        };
        let table: toml::Table = text.parse().map_err(|e: toml::de::Error| format!("{}: {e}", path.display()))?;
        let text_of = |t: &toml::Table, k: &str| t.get(k).and_then(toml::Value::as_str).unwrap_or("").to_string();
        let date_of = |t: &toml::Table, k: &str| t.get(k).and_then(|v| v.as_datetime().map(|d| d.to_string()).or_else(|| v.as_str().map(str::to_string))).and_then(|s| s.get(..10).and_then(|s| s.parse().ok()));
        let list = table
            .get("letter")
            .and_then(toml::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|v| {
                let t = v.as_table()?;
                let time = t.get("time").and_then(toml::Value::as_str).and_then(|s| s.split_once(':')).and_then(|(h, m)| Some((h.parse().ok()?, m.parse().ok()?)));
                Some(Letter {
                    id: text_of(t, "id"),
                    file: text_of(t, "file"),
                    source: text_of(t, "source"),
                    received: date_of(t, "received"),
                    reading: Reading {
                        sender: text_of(t, "sender"),
                        kind: Kind::of(&text_of(t, "kind")),
                        amount: t.get("amount").and_then(|v| v.as_float().or_else(|| v.as_integer().map(|i| i as f64))).map(Money::from_units),
                        dated: date_of(t, "dated"),
                        deadline: date_of(t, "deadline"),
                        why: text_of(t, "why"),
                        appointment: date_of(t, "appointment").map(|d| (d, time)),
                        registered: t.get("registered").and_then(toml::Value::as_bool).unwrap_or(false),
                        reference: text_of(t, "reference"),
                    },
                    case: text_of(t, "case"),
                    status: text_of(t, "status"),
                    task: text_of(t, "task"),
                    problem: text_of(t, "problem"),
                })
            })
            .collect();
        Ok(Letters { root: root.to_path_buf(), list })
    }

    pub fn get(&self, id: &str) -> Option<&Letter> {
        self.list.iter().find(|l| l.id == id)
    }

    pub fn put(&mut self, letter: Letter) {
        match self.list.iter_mut().find(|l| l.id == letter.id) {
            Some(place) => *place = letter,
            None => self.list.push(letter),
        }
    }

    /// A scan's file, where it is on this computer.
    pub fn file_path(&self, letter: &Letter) -> PathBuf {
        let path = Path::new(&letter.file);
        if path.is_absolute() { path.to_path_buf() } else { self.root.join(path) }
    }

    /// Its text, as read.
    pub fn text(&self, letter: &Letter) -> String {
        std::fs::read_to_string(Letters::folder(&self.root).join(format!("{}.txt", letter.id))).unwrap_or_default()
    }

    pub fn save_text(&self, id: &str, text: &str) -> Result<(), String> {
        let folder = Letters::folder(&self.root);
        std::fs::create_dir_all(&folder).and_then(|()| std::fs::write(folder.join(format!("{id}.txt")), text)).map_err(|e| e.to_string())
    }

    /// Its scan moved out of the inbox into `letters/<year>/`, named by its day, sender and kind.
    pub fn file_away(&self, letter: &mut Letter) -> Result<(), String> {
        let from = self.file_path(letter);
        let day = letter.reading.dated.or(letter.received).unwrap_or(Date::constant(2000, 1, 1));
        let folder = Letters::folder(&self.root).join(day.year().to_string());
        if from.starts_with(&folder) {
            return Ok(());
        }
        std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
        let extension = from.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
        let sender: String = crate::cases::new_id(&letter.reading.sender, &[]);
        let base = format!("{day} {sender} {}", letter.reading.kind.id());
        let target = std::iter::once(folder.join(format!("{base}{extension}"))).chain((2..1000).map(|n| folder.join(format!("{base} {n}{extension}")))).find(|p| !p.exists()).ok_or("no free name")?;
        std::fs::rename(&from, &target).or_else(|_| std::fs::copy(&from, &target).and_then(|_| std::fs::remove_file(&from))).map_err(|e| e.to_string())?;
        letter.file = target.strip_prefix(&self.root).map_or_else(|_| target.display().to_string(), |p| p.to_string_lossy().replace('\\', "/"));
        Ok(())
    }

    pub fn save(&self) -> Result<(), String> {
        let folder = Letters::folder(&self.root);
        std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
        let mut doc = toml_edit::DocumentMut::new();
        doc.decor_mut().set_prefix("# Paper letters, read from their scans (docs/porch.md). Their texts are beside: <id>.txt.\n\n");
        let date = |d: Date| toml_edit::value(toml_edit::Datetime { date: Some(toml_edit::Date { year: d.year() as u16, month: d.month() as u8, day: d.day() as u8 }), time: None, offset: None });
        let mut list = toml_edit::ArrayOfTables::new();
        for l in &self.list {
            let mut t = toml_edit::Table::new();
            t["id"] = toml_edit::value(&l.id);
            t["file"] = toml_edit::value(&l.file);
            for (k, v) in [("source", &l.source), ("sender", &l.reading.sender), ("why", &l.reading.why), ("reference", &l.reading.reference), ("case", &l.case), ("status", &l.status), ("task", &l.task), ("problem", &l.problem)] {
                if !v.is_empty() {
                    t[k] = toml_edit::value(v);
                }
            }
            t["kind"] = toml_edit::value(l.reading.kind.id());
            if let Some(amount) = l.reading.amount {
                t["amount"] = toml_edit::value(amount.cents() as f64 / 100.0);
            }
            for (k, d) in [("received", l.received), ("dated", l.reading.dated), ("deadline", l.reading.deadline), ("appointment", l.reading.appointment.map(|(d, _)| d))] {
                if let Some(d) = d {
                    t[k] = date(d);
                }
            }
            if let Some((_, Some((h, m)))) = l.reading.appointment {
                t["time"] = toml_edit::value(format!("{h:02}:{m:02}"));
            }
            if l.reading.registered {
                t["registered"] = toml_edit::value(true);
            }
            list.push(t);
        }
        doc.insert("letter", toml_edit::Item::ArrayOfTables(list));
        let path = folder.join(MANIFEST);
        let temporary = folder.join(format!("{MANIFEST}.new"));
        std::fs::write(&temporary, doc.to_string()).and_then(|()| std::fs::rename(&temporary, &path)).map_err(|e| format!("{}: {e}", path.display()))
    }
}

/// The project a letter belongs with: the one whose name's words it carries most.
pub fn case_of(text: &str, sender: &str, cases: &[crate::cases::Case]) -> Option<String> {
    let all = plain(&format!("{sender} {text}"));
    cases
        .iter()
        .filter(|c| c.status.as_deref() != Some("closed"))
        .map(|c| {
            let words: Vec<String> = plain(&c.title).split_whitespace().filter(|w| w.len() >= 4).map(str::to_string).collect();
            (words.iter().filter(|w| all.contains(&format!(" {w} "))).count(), words.len(), c.id.clone())
        })
        .filter(|(hits, words, _)| *hits > 0 && *hits * 2 >= *words)
        .max_by_key(|(hits, _, _)| *hits)
        .map(|(_, _, id)| id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(text: &str) -> Date {
        text.parse().unwrap()
    }

    #[test]
    fn a_decision_with_its_appeal_delay() {
        let text = "CAISSE D'ALLOCATIONS FAMILIALES\nd'Exempleville\n\nExempleville, le 28 septembre 2026\n\nNuméro allocataire : 1234567 X\n\nObjet : notification de décision\n\nMadame, Monsieur,\nNous avons examiné votre demande d'aide au logement. Notre décision : votre droit est refusé.\n\nVoies et délais de recours : vous pouvez contester cette décision dans un délai de deux mois à compter de sa notification, auprès de la commission de recours amiable.\n";
        let r = read(text, day("2026-10-02"));
        assert_eq!(r.sender, "CAF");
        assert_eq!(r.kind, Kind::Decision);
        assert_eq!(r.dated, Some(day("2026-09-28")));
        assert_eq!(r.deadline, Some(day("2026-12-02")), "two months from the day it came");
        assert!(r.why.contains("délai de deux mois à compter"), "as written: {}", r.why);
        assert_eq!(r.reference, "1234567");
    }

    #[test]
    fn a_bill_with_its_date_and_amount() {
        let text = "CENTRE HOSPITALIER D'EXEMPLEVILLE\nService des recettes\n\nAVIS DES SOMMES A PAYER\n\nDate d'émission : 15/09/2026\nRéférence : 2026-ASAP-01234\n\nMontant à payer : 1 234,56 €\nÀ régler avant le 15 novembre 2026, par virement ou en ligne sur payfip.gouv.fr.\n\nLettre recommandée avec accusé de réception\n";
        let r = read(text, day("2026-10-02"));
        assert_eq!(r.sender, "Hôpital");
        assert_eq!(r.kind, Kind::Bill);
        assert_eq!(r.amount, Some(Money(123456)));
        assert_eq!(r.deadline, Some(day("2026-11-15")));
        assert!(r.registered);
        assert_eq!(r.reference, "2026-ASAP-01234");
    }

    #[test]
    fn a_formal_notice_comes_first_and_eight_days() {
        let text = "Cabinet Exemple\nCommissaire de justice\n\nMISE EN DEMEURE\n\nNous vous mettons en demeure de régler la somme de 2 345,00 € sous huitaine.\n";
        let r = read(text, day("2026-10-02"));
        assert_eq!(r.kind, Kind::FormalNotice);
        assert_eq!(r.sender, "Commissaire de justice");
        assert_eq!(r.amount, Some(Money(234500)));
        assert_eq!(r.deadline, Some(day("2026-10-10")));
    }

    #[test]
    fn an_appointment_with_its_hour() {
        let text = "Assurance Maladie\nCPAM d'Exempleville\n\nConvocation\n\nVous êtes convoqué à un examen le jeudi 22 octobre 2026 à 9 h 15, au centre d'examens de santé.\n";
        let r = read(text, day("2026-10-02"));
        assert_eq!(r.kind, Kind::Appointment);
        assert_eq!(r.sender, "Assurance Maladie");
        assert_eq!(r.appointment, Some((day("2026-10-22"), Some((9, 15)))));
    }

    #[test]
    fn english_and_unknown_senders() {
        let text = "Example Water Ltd\n12 High Street\n\nInvoice\nAmount due: £84.20\nPlease pay no later than 30 October 2026.\n";
        let r = read(text, day("2026-10-02"));
        assert_eq!(r.sender, "Example Water Ltd");
        assert_eq!(r.kind, Kind::Bill);
        assert_eq!(r.deadline, Some(day("2026-10-30")));
        assert_eq!(r.amount, Some(Money(8420)));
    }

    #[test]
    fn kept_and_filed() {
        let root = std::env::temp_dir().join(format!("sioul-letters-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("letters/inbox")).unwrap();
        std::fs::write(root.join("letters/inbox/scan.pdf"), b"%PDF").unwrap();
        let mut letters = Letters::load(&root).unwrap();
        let mut letter = Letter { id: "a1".into(), file: "letters/inbox/scan.pdf".into(), received: Some(day("2026-10-02")), status: "new".into(), ..Letter::default() };
        letter.reading = read("CAF\nExempleville, le 28 septembre 2026\nDécision. Voies et délais de recours : recours dans un délai de deux mois.", day("2026-10-02"));
        letters.file_away(&mut letter).unwrap();
        assert_eq!(letter.file, "letters/2026/2026-09-28 caf decision.pdf");
        assert!(root.join(&letter.file).is_file() && !root.join("letters/inbox/scan.pdf").exists());
        letters.put(letter.clone());
        letters.save_text("a1", "the text").unwrap();
        letters.save().unwrap();
        let again = Letters::load(&root).unwrap();
        assert_eq!(again.list, vec![letter]);
        assert_eq!(again.text(&again.list[0]), "the text");
        let _ = std::fs::remove_dir_all(&root);
    }
}

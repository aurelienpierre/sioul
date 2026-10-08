// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The papers wallet (docs/papers.md): the papers asked again and again (an
//! identity card, the last tax notice, bank details, rent receipts, an
//! attestation), each with its file and how long it holds; a reminder before
//! one ends, early enough to renew it (a passport takes weeks, the
//! complémentaire santé solidaire does not renew by itself).
//!
//! `sioul-papers.toml` at the root of the notes folder, the files in its
//! `papers/` folder: they travel with your projects and notes.

use jiff::civil::Date;
use jiff::{Span, ToSpan};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const MANIFEST: &str = "sioul-papers.toml";
pub const FOLDER: &str = "papers";

/// What a paper is.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    Identity,
    Passport,
    Residence,
    Driving,
    HealthCard,
    HealthCover,
    Insurance,
    TaxNotice,
    RentReceipt,
    BankDetails,
    Payslip,
    Certificate,
    Warranty,
    #[default]
    Other,
}

impl Kind {
    pub const ALL: [Kind; 14] = [
        Kind::Identity,
        Kind::Passport,
        Kind::Residence,
        Kind::Driving,
        Kind::HealthCard,
        Kind::HealthCover,
        Kind::Insurance,
        Kind::TaxNotice,
        Kind::RentReceipt,
        Kind::BankDetails,
        Kind::Payslip,
        Kind::Certificate,
        Kind::Warranty,
        Kind::Other,
    ];

    /// "identity", "health-cover": its name in the file and in the strings (`paper-kind-<id>`).
    pub fn id(self) -> &'static str {
        match self {
            Kind::Identity => "identity",
            Kind::Passport => "passport",
            Kind::Residence => "residence",
            Kind::Driving => "driving",
            Kind::HealthCard => "health-card",
            Kind::HealthCover => "health-cover",
            Kind::Insurance => "insurance",
            Kind::TaxNotice => "tax-notice",
            Kind::RentReceipt => "rent-receipt",
            Kind::BankDetails => "bank-details",
            Kind::Payslip => "payslip",
            Kind::Certificate => "certificate",
            Kind::Warranty => "warranty",
            Kind::Other => "other",
        }
    }

    pub fn of(id: &str) -> Kind {
        Kind::ALL.into_iter().find(|k| k.id() == id).unwrap_or_default()
    }

    /// Where it is shown: identity, health, home, money, warranties, others (`paper-family-<id>`).
    pub fn family(self) -> &'static str {
        match self {
            Kind::Identity | Kind::Passport | Kind::Residence | Kind::Driving => "identity",
            Kind::HealthCard | Kind::HealthCover => "health",
            Kind::Insurance | Kind::RentReceipt => "home",
            Kind::TaxNotice | Kind::BankDetails | Kind::Payslip => "money",
            Kind::Warranty => "warranty",
            Kind::Certificate | Kind::Other => "other",
        }
    }

    /// How many days before it ends renewing starts: weeks of waiting for an
    /// appointment and the making of a passport or a card; a residence permit
    /// asked from four months before; the complémentaire santé solidaire two to
    /// four months before, as it does not renew by itself. None: a newer one comes on its own.
    pub fn lead_days(self) -> Option<i64> {
        match self {
            Kind::Identity | Kind::Passport | Kind::HealthCover => Some(90),
            Kind::Residence => Some(120),
            Kind::Driving => Some(60),
            Kind::Insurance | Kind::Warranty => Some(30),
            _ => None,
        }
    }

    /// Asked "less than three months old" when sent (rent receipts, payslips,
    /// attestations); a tax notice is the last one, about a year.
    pub fn fresh_days(self) -> Option<i64> {
        match self {
            Kind::RentReceipt | Kind::Payslip | Kind::Certificate => Some(92),
            Kind::TaxNotice => Some(400),
            _ => None,
        }
    }

    /// How long it usually holds from its issue, in years, to propose an end:
    /// a French passport or identity card for an adult, ten years (cards made
    /// from 2021); the legal guarantee of conformity, two years from delivery.
    pub fn years(self) -> Option<i16> {
        match self {
            Kind::Identity | Kind::Passport => Some(10),
            Kind::Warranty => Some(2),
            _ => None,
        }
    }

    /// The kind a file's name or a mail's subject suggests: "avis_impot_2026.pdf"
    /// → a tax notice, by the words of each kind (`words::PaperKinds`), the
    /// kinds tried in this order.
    pub fn guess(words: &crate::words::Words, text: &str) -> Option<Kind> {
        // Folded, punctuation as spaces: "Avis_d'impôt-2026.pdf" → "avis d impot 2026 pdf".
        let plain = |text: &str| crate::text::fold(text).into_iter().map(|c| if c.is_alphanumeric() { c } else { ' ' }).collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ");
        let padded = format!(" {} ", plain(text));
        let has = |list: &[String]| list.iter().map(|w| plain(w)).any(|w| !w.is_empty() && padded.contains(&format!(" {w} ")));
        let k = &words.papers.kinds;
        let kind = if has(&k.passport) {
            Kind::Passport
        } else if has(&k.identity) {
            Kind::Identity
        } else if has(&k.residence) {
            Kind::Residence
        } else if has(&k.driving) {
            Kind::Driving
        } else if has(&k.health_cover) {
            Kind::HealthCover
        } else if has(&k.health_card) {
            Kind::HealthCard
        } else if has(&k.tax_notice) {
            Kind::TaxNotice
        } else if has(&k.rent_receipt) {
            Kind::RentReceipt
        } else if has(&k.bank_details) {
            Kind::BankDetails
        } else if has(&k.payslip) {
            Kind::Payslip
        } else if has(&k.warranty) {
            Kind::Warranty
        } else if has(&k.insurance) {
            Kind::Insurance
        } else if has(&k.certificate) {
            Kind::Certificate
        } else {
            return None;
        };
        Some(kind)
    }
}

/// One paper.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct Paper {
    pub id: String,
    #[serde(default)]
    pub kind: Kind,
    pub title: String,
    /// Its file: in the notes folder (`papers/…`), or anywhere.
    #[serde(default)]
    pub file: String,
    #[serde(default, deserialize_with = "crate::budget::dates::optional")]
    pub issued: Option<Date>,
    /// The last day it holds.
    #[serde(default, deserialize_with = "crate::budget::dates::optional")]
    pub until: Option<Date>,
    /// Whose, in a household.
    #[serde(default)]
    pub holder: String,
    #[serde(default)]
    pub notes: String,
    /// The task made to renew it, by UID.
    #[serde(default)]
    pub renewal: String,
    /// When it entered the wallet: one added after its renewal began is not reminded, you just saw it.
    #[serde(default, deserialize_with = "crate::budget::dates::optional")]
    pub added: Option<Date>,
}

/// Where a paper stands, today.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase", tag = "is", content = "date")]
pub enum Standing {
    /// Holds until then.
    Valid(Date),
    /// Holds until then, and renewing starts now.
    Renew(Date),
    /// Ended then.
    Ended(Date),
    /// Recent enough to send: issued then.
    Fresh(Date),
    /// Older than what is usually asked: issued then.
    Old(Date),
    /// Without a date that matters.
    Undated,
}

impl Paper {
    pub fn standing(&self, today: Date) -> Standing {
        if let Some(until) = self.until {
            if until < today {
                return Standing::Ended(until);
            }
            let starts = self.kind.lead_days().and_then(|d| until.checked_sub(d.days()).ok());
            return if starts.is_some_and(|s| s <= today) { Standing::Renew(until) } else { Standing::Valid(until) };
        }
        match (self.issued, self.kind.fresh_days()) {
            (Some(issued), Some(days)) if issued.checked_add(days.days()).is_ok_and(|limit| limit < today) => Standing::Old(issued),
            (Some(issued), Some(_)) => Standing::Fresh(issued),
            _ => Standing::Undated,
        }
    }

    /// When renewing starts, if it is renewed.
    pub fn renew_from(&self) -> Option<Date> {
        let until = self.until?;
        until.checked_sub(Span::new().days(self.kind.lead_days()?)).ok()
    }
}

/// A file name from elsewhere (an attachment's), as every system takes it:
/// the folder travels to Windows and macOS too. Its last part only, back
/// slashes included; what Windows refuses (":" would even write into a
/// hidden stream) as "-"; no final dot or space; a device name ("CON",
/// "NUL"…) with a "_" before it; "paper" when nothing is left.
fn portable_name(name: &str) -> String {
    let last = name.rsplit(['/', '\\']).next().unwrap_or("");
    let clean: String = last.chars().map(|c| if c.is_control() || matches!(c, ':' | '*' | '?' | '"' | '<' | '>' | '|') { '-' } else { c }).collect();
    let clean = clean.trim().trim_end_matches(['.', ' ']).to_string();
    if clean.is_empty() {
        return "paper".into();
    }
    let stem = clean.split('.').next().unwrap_or("").trim().to_ascii_uppercase();
    let device = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL") || (stem.len() == 4 && (stem.starts_with("COM") || stem.starts_with("LPT")) && stem.as_bytes()[3].is_ascii_digit());
    if device { format!("_{clean}") } else { clean }
}

/// An end proposed from the issue, when the kind's usual length is known.
pub fn usual_end(kind: Kind, issued: Date) -> Option<Date> {
    issued.checked_add(Span::new().years(kind.years()?)).ok()?.yesterday().ok()
}

/// The papers of a notes folder.
#[derive(Debug, Clone, Default)]
pub struct Wallet {
    pub root: PathBuf,
    pub papers: Vec<Paper>,
}

#[derive(Deserialize, Default)]
struct File {
    #[serde(default, rename = "paper")]
    papers: Vec<Paper>,
}

impl Wallet {
    /// The wallet at the root of a notes folder; empty when there is none yet.
    pub fn load(root: &Path) -> Result<Wallet, String> {
        let path = root.join(MANIFEST);
        let papers = match std::fs::read_to_string(&path) {
            Ok(text) => toml::from_str::<File>(&text).map_err(|e| format!("{}: {e}", path.display()))?.papers,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(e) => return Err(format!("{}: {e}", path.display())),
        };
        Ok(Wallet { root: root.to_path_buf(), papers })
    }

    pub fn get(&self, id: &str) -> Option<&Paper> {
        self.papers.iter().find(|p| p.id == id)
    }

    /// A paper's file, where it is on this computer.
    pub fn file_path(&self, paper: &Paper) -> Option<PathBuf> {
        if paper.file.is_empty() {
            return None;
        }
        let path = Path::new(&paper.file);
        Some(if path.is_absolute() { path.to_path_buf() } else { self.root.join(path) })
    }

    /// A paper added or changed (by id; a new one gets an id from its title); returns its id.
    pub fn put(&mut self, mut paper: Paper) -> String {
        if paper.id.is_empty() {
            let taken: Vec<String> = self.papers.iter().map(|p| p.id.clone()).collect();
            paper.id = crate::projects::new_id(&paper.title, &taken);
        }
        let id = paper.id.clone();
        match self.papers.iter_mut().find(|p| p.id == id) {
            Some(place) => *place = paper,
            None => self.papers.push(paper),
        }
        id
    }

    /// A paper taken out of the wallet; its file stays where it is.
    pub fn remove(&mut self, id: &str) -> bool {
        let before = self.papers.len();
        self.papers.retain(|p| p.id != id);
        self.papers.len() != before
    }

    /// A file copied into the wallet's folder, under a free name; its path from the notes folder.
    pub fn keep_file(&self, source: &Path, name: &str) -> Result<String, String> {
        let folder = self.root.join(FOLDER);
        std::fs::create_dir_all(&folder).map_err(|e| format!("{}: {e}", folder.display()))?;
        if source.starts_with(&folder) {
            return Ok(source.strip_prefix(&self.root).unwrap_or(source).to_string_lossy().replace('\\', "/"));
        }
        let name = portable_name(name);
        let (stem, extension) = match name.rsplit_once('.') {
            Some((stem, extension)) => (stem.to_string(), format!(".{extension}")),
            None => (name.clone(), String::new()),
        };
        let target = std::iter::once(folder.join(&name)).chain((2..1000).map(|n| folder.join(format!("{stem}-{n}{extension}")))).find(|p| !p.exists()).ok_or("no free name")?;
        std::fs::copy(source, &target).map_err(|e| format!("{}: {e}", target.display()))?;
        Ok(format!("{FOLDER}/{}", target.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()))
    }

    /// Written whole, next to its place then moved: never half a file.
    pub fn save(&self) -> Result<(), String> {
        let mut doc = toml_edit::DocumentMut::new();
        doc.decor_mut().set_prefix("# Your papers: what each is, its file, how long it holds (docs/papers.md).\n\n");
        let mut list = toml_edit::ArrayOfTables::new();
        let date = |d: Date| toml_edit::value(toml_edit::Datetime { date: Some(toml_edit::Date { year: d.year() as u16, month: d.month() as u8, day: d.day() as u8 }), time: None, offset: None });
        for paper in &self.papers {
            let mut table = toml_edit::Table::new();
            table["id"] = toml_edit::value(&paper.id);
            table["kind"] = toml_edit::value(paper.kind.id());
            table["title"] = toml_edit::value(&paper.title);
            for (name, text) in [("file", &paper.file), ("holder", &paper.holder), ("notes", &paper.notes), ("renewal", &paper.renewal)] {
                if !text.is_empty() {
                    table[name] = toml_edit::value(text);
                }
            }
            if let Some(d) = paper.issued {
                table["issued"] = date(d);
            }
            if let Some(d) = paper.until {
                table["until"] = date(d);
            }
            if let Some(d) = paper.added {
                table["added"] = date(d);
            }
            list.push(table);
        }
        doc.insert("paper", toml_edit::Item::ArrayOfTables(list));
        let path = self.root.join(MANIFEST);
        let temporary = self.root.join(format!("{MANIFEST}.new"));
        std::fs::write(&temporary, doc.to_string()).and_then(|()| std::fs::rename(&temporary, &path)).map_err(|e| format!("{}: {e}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(text: &str) -> Date {
        text.parse().unwrap()
    }

    #[test]
    fn where_each_stands() {
        let today = day("2026-10-03");
        let passport = Paper { kind: Kind::Passport, until: Some(day("2026-12-20")), ..Paper::default() };
        assert_eq!(passport.standing(today), Standing::Renew(day("2026-12-20")), "within its three months");
        assert_eq!(passport.renew_from(), Some(day("2026-09-21")));
        let later = Paper { until: Some(day("2027-06-01")), ..passport.clone() };
        assert_eq!(later.standing(today), Standing::Valid(day("2027-06-01")));
        let ended = Paper { until: Some(day("2026-09-30")), ..passport };
        assert_eq!(ended.standing(today), Standing::Ended(day("2026-09-30")));
        let receipt = Paper { kind: Kind::RentReceipt, issued: Some(day("2026-06-01")), ..Paper::default() };
        assert_eq!(receipt.standing(today), Standing::Old(day("2026-06-01")));
        let fresh = Paper { issued: Some(day("2026-09-01")), ..receipt };
        assert_eq!(fresh.standing(today), Standing::Fresh(day("2026-09-01")));
        assert_eq!(Paper { kind: Kind::BankDetails, ..Paper::default() }.standing(today), Standing::Undated);
        assert_eq!(usual_end(Kind::Passport, day("2017-03-12")), Some(day("2027-03-11")));
        assert_eq!(usual_end(Kind::Warranty, day("2026-10-03")), Some(day("2028-10-02")));
    }

    #[test]
    fn guessed_from_names() {
        assert_eq!(Kind::guess(&crate::words::Words::builtin(), "avis_impot_2026.pdf"), Some(Kind::TaxNotice));
        assert_eq!(Kind::guess(&crate::words::Words::builtin(), "Votre quittance de loyer — septembre"), Some(Kind::RentReceipt));
        assert_eq!(Kind::guess(&crate::words::Words::builtin(), "RIB Ma Banque.pdf"), Some(Kind::BankDetails));
        assert_eq!(Kind::guess(&crate::words::Words::builtin(), "Attestation de droits à l'Assurance Maladie"), Some(Kind::HealthCard));
        assert_eq!(Kind::guess(&crate::words::Words::builtin(), "Passeport.jpg"), Some(Kind::Passport));
        assert_eq!(Kind::guess(&crate::words::Words::builtin(), "holiday.jpg"), None);
        assert_eq!(Kind::guess(&crate::words::Words::builtin(), "Carte d'identité recto.jpg"), Some(Kind::Identity));
        assert_eq!(Kind::guess(&crate::words::Words::builtin(), "Votre avis d’impôt 2026 est disponible"), Some(Kind::TaxNotice));
        assert_eq!(Kind::guess(&crate::words::Words::builtin(), "cassis.pdf"), None, "a word inside another is not it: css");
    }

    #[test]
    fn kept_and_read_again() {
        let root = std::env::temp_dir().join(format!("sioul-papers-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let scan = root.join("scan.pdf");
        std::fs::write(&scan, b"%PDF-1.4").unwrap();
        let mut wallet = Wallet::load(&root).unwrap();
        assert!(wallet.papers.is_empty());
        let file = wallet.keep_file(&scan, "Passeport.pdf").unwrap();
        assert_eq!(file, "papers/Passeport.pdf");
        assert_eq!(wallet.keep_file(&scan, "Passeport.pdf").unwrap(), "papers/Passeport-2.pdf", "a free name");
        // An attachment's name from elsewhere: its last part, as every system takes it.
        assert_eq!(wallet.keep_file(&scan, "..\\..\\avis:2026?.pdf").unwrap(), "papers/avis-2026-.pdf");
        assert_eq!(wallet.keep_file(&scan, "CON.pdf").unwrap(), "papers/_CON.pdf");
        assert_eq!(wallet.keep_file(&scan, "..").unwrap(), "papers/paper");
        let id = wallet.put(Paper { kind: Kind::Passport, title: "Passeport".into(), file, until: Some(day("2027-03-11")), ..Paper::default() });
        assert_eq!(id, "passeport");
        wallet.save().unwrap();
        let again = Wallet::load(&root).unwrap();
        assert_eq!(again.papers, wallet.papers);
        assert!(std::fs::read_to_string(root.join(MANIFEST)).unwrap().contains("until = 2027-03-11"));
        assert!(again.file_path(&again.papers[0]).unwrap().is_file());
        let mut again = again;
        assert!(again.remove("passeport") && again.papers.is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }
}

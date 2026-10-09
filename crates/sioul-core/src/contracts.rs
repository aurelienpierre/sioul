// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Contracts and subscriptions (docs/accounting.md, "Contracts"): what you are
//! bound to (rent, energy, phone, insurances, hosting, subscriptions), when
//! each renews, the notice it needs, how to stop it, what it covers. Found
//! from the recurring payments of your budgets and from mail; a reminder comes
//! before the last day to send a notice; "Cancel" opens the provider's own
//! cancel page (in France a three-click button is required online since
//! June 2023, loi 2022-1158) or drafts the letter. Facts about your contracts,
//! never offers.
//!
//! `sioul-contracts.toml` at the root of the notes folder.

use jiff::civil::Date;
use jiff::{Span, ToSpan};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const MANIFEST: &str = "sioul-contracts.toml";

/// What a contract is.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    Rent,
    Energy,
    Telecom,
    Insurance,
    Health,
    Subscription,
    Hosting,
    Bank,
    #[default]
    Other,
}

impl Kind {
    pub const ALL: [Kind; 9] = [Kind::Rent, Kind::Energy, Kind::Telecom, Kind::Insurance, Kind::Health, Kind::Subscription, Kind::Hosting, Kind::Bank, Kind::Other];

    pub fn id(self) -> &'static str {
        match self {
            Kind::Rent => "rent",
            Kind::Energy => "energy",
            Kind::Telecom => "telecom",
            Kind::Insurance => "insurance",
            Kind::Health => "health",
            Kind::Subscription => "subscription",
            Kind::Hosting => "hosting",
            Kind::Bank => "bank",
            Kind::Other => "other",
        }
    }

    pub fn of(id: &str) -> Kind {
        Kind::ALL.into_iter().find(|k| k.id() == id).unwrap_or_default()
    }

    /// The notice usually asked, in days, to propose when none is written; you
    /// change it to what your contract says. A tenant gives three months (one
    /// in a furnished flat or a tight area); insurances and health covers after
    /// their first year one month (loi Hamon, and since December 2020 for health
    /// covers); phone and internet after the first twelve months at most ten
    /// days; energy, none (art. L224-13 of the consumer code: change at any time).
    pub fn usual_notice(self) -> u32 {
        match self {
            Kind::Rent => 90,
            Kind::Insurance | Kind::Health => 30,
            Kind::Telecom => 10,
            _ => 0,
        }
    }

    /// The kind a payment's label, a mail's subject or a sender's name
    /// suggests, by the words and brands of each kind (`words::ContractKinds`:
    /// the language packs' words, the country packs' brands, yours), the
    /// kinds tried in this order.
    pub fn guess(words: &crate::words::Words, text: &str) -> Option<Kind> {
        let folded: String = crate::text::fold(text).into_iter().collect();
        let has = |list: &[String]| {
            list.iter().map(|w| crate::words::folded(w)).any(|w| !w.is_empty() && (folded.split(|c: char| !c.is_alphanumeric()).any(|word| word == w) || (w.contains(' ') && folded.contains(&w))))
        };
        let k = &words.contracts.kinds;
        let kind = if has(&k.rent) {
            Kind::Rent
        } else if has(&k.energy) {
            Kind::Energy
        } else if has(&k.health) {
            Kind::Health
        } else if has(&k.insurance) {
            Kind::Insurance
        } else if has(&k.telecom) {
            Kind::Telecom
        } else if has(&k.hosting) {
            Kind::Hosting
        } else if has(&k.subscription) {
            Kind::Subscription
        } else if has(&k.bank) {
            Kind::Bank
        } else {
            return None;
        };
        Some(kind)
    }
}

/// One contract.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct Contract {
    pub id: String,
    #[serde(default)]
    pub kind: Kind,
    pub title: String,
    /// Who it is with: "EDF", "MAIF".
    #[serde(default)]
    pub party: String,
    /// Your customer or contract number with them.
    #[serde(default)]
    pub reference: String,
    /// The budget preset that pays it, by id.
    #[serde(default)]
    pub preset: String,
    #[serde(default, deserialize_with = "crate::budget::dates::optional")]
    pub started: Option<Date>,
    /// When it renews by itself (its anniversary), if it does.
    #[serde(default, deserialize_with = "crate::budget::dates::optional")]
    pub renews: Option<Date>,
    /// "year", "month", or "" when it does not renew on a date.
    #[serde(default)]
    pub every: String,
    /// Days of notice before a renewal (or before it ends, when it runs on).
    #[serde(default)]
    pub notice_days: u32,
    /// How to stop it: its cancel page ("https://…") or an address to write to.
    #[serde(default)]
    pub cancel: String,
    /// What it covers (an insurance: legal protection, liability…).
    #[serde(default)]
    pub covers: String,
    #[serde(default)]
    pub notes: String,
    /// The paper holding it, in the papers wallet.
    #[serde(default)]
    pub paper: String,
    /// It ended then: kept as a record.
    #[serde(default, deserialize_with = "crate::budget::dates::optional")]
    pub ended: Option<Date>,
}

impl Contract {
    /// Its next renewal on or after `today`: its date, moved on by its term.
    /// Each turn is counted from the date written, so the 31st stays the 31st
    /// after a short month (the 30th in April, the 31st again in May).
    pub fn next_renewal(&self, today: Date) -> Option<Date> {
        let first = self.renews?;
        let turns = |n: i64| match self.every.as_str() {
            "year" => Some(Span::new().years(n)),
            "month" => Some(Span::new().months(n)),
            _ => None,
        };
        if turns(0).is_none() {
            return (first >= today).then_some(first);
        }
        for n in 0..1200 {
            let at = first.checked_add(turns(n)?).ok()?;
            if at >= today {
                return Some(at);
            }
        }
        None
    }

    /// The last day a notice can leave to stop the next renewal.
    pub fn cancel_by(&self, today: Date) -> Option<Date> {
        let renewal = self.next_renewal(today)?;
        renewal.checked_sub((i64::from(self.notice_days)).days()).ok()
    }

    pub fn is_open(&self) -> bool {
        self.ended.is_none()
    }
}

/// The contracts of a notes folder.
#[derive(Debug, Clone, Default)]
pub struct Contracts {
    pub root: PathBuf,
    pub list: Vec<Contract>,
}

#[derive(Deserialize, Default)]
struct File {
    #[serde(default, rename = "contract")]
    list: Vec<Contract>,
}

impl Contracts {
    /// Read, changed by `change` and written back, all under the file's lock,
    /// which the sharing takes too: a change is set over the file as it is
    /// then, never over a copy read earlier. Returns what `change` returns.
    pub fn change<R>(root: &Path, change: impl FnOnce(&mut Contracts) -> R) -> Result<R, String> {
        crate::filelock::with_lock(&root.join(MANIFEST), || {
            let mut contracts = Contracts::load(root)?;
            let out = change(&mut contracts);
            contracts.save()?;
            Ok(out)
        })
    }

    pub fn load(root: &Path) -> Result<Contracts, String> {
        let path = root.join(MANIFEST);
        let list = match std::fs::read_to_string(&path) {
            Ok(text) => toml::from_str::<File>(&text).map_err(|e| format!("{}: {e}", path.display()))?.list,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(e) => return Err(format!("{}: {e}", path.display())),
        };
        Ok(Contracts { root: root.to_path_buf(), list })
    }

    pub fn get(&self, id: &str) -> Option<&Contract> {
        self.list.iter().find(|c| c.id == id)
    }

    /// Added or changed (by id; a new one gets an id from its title); its id.
    pub fn put(&mut self, mut contract: Contract) -> String {
        if contract.id.is_empty() {
            let taken: Vec<String> = self.list.iter().map(|c| c.id.clone()).collect();
            contract.id = crate::projects::new_id(&contract.title, &taken);
        }
        let id = contract.id.clone();
        match self.list.iter_mut().find(|c| c.id == id) {
            Some(place) => *place = contract,
            None => self.list.push(contract),
        }
        id
    }

    pub fn remove(&mut self, id: &str) -> bool {
        let before = self.list.len();
        self.list.retain(|c| c.id != id);
        before != self.list.len()
    }

    /// Written whole, next to its place then moved, under the file's lock.
    pub fn save(&self) -> Result<(), String> {
        crate::filelock::with_lock(&self.root.join(MANIFEST), || self.write())
    }

    fn write(&self) -> Result<(), String> {
        let mut doc = toml_edit::DocumentMut::new();
        doc.decor_mut().set_prefix("# What you are bound to: renewals, notices, how to stop each (docs/accounting.md).\n\n");
        let date = |d: Date| toml_edit::value(toml_edit::Datetime { date: Some(toml_edit::Date { year: d.year() as u16, month: d.month() as u8, day: d.day() as u8 }), time: None, offset: None });
        let mut list = toml_edit::ArrayOfTables::new();
        for c in &self.list {
            let mut t = toml_edit::Table::new();
            t["id"] = toml_edit::value(&c.id);
            t["kind"] = toml_edit::value(c.kind.id());
            t["title"] = toml_edit::value(&c.title);
            for (name, text) in [("party", &c.party), ("reference", &c.reference), ("preset", &c.preset), ("every", &c.every), ("cancel", &c.cancel), ("covers", &c.covers), ("notes", &c.notes), ("paper", &c.paper)] {
                if !text.is_empty() {
                    t[name] = toml_edit::value(text);
                }
            }
            if c.notice_days > 0 {
                t["notice_days"] = toml_edit::value(i64::from(c.notice_days));
            }
            for (name, day) in [("started", c.started), ("renews", c.renews), ("ended", c.ended)] {
                if let Some(d) = day {
                    t[name] = date(d);
                }
            }
            list.push(t);
        }
        doc.insert("contract", toml_edit::Item::ArrayOfTables(list));
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
    fn renewals_and_notices() {
        let today = day("2026-10-03");
        let home = Contract { kind: Kind::Insurance, renews: Some(day("2025-12-01")), every: "year".into(), notice_days: 60, ..Contract::default() };
        assert_eq!(home.next_renewal(today), Some(day("2026-12-01")));
        assert_eq!(home.cancel_by(today), Some(day("2026-10-02")), "two months before");
        let phone = Contract { renews: Some(day("2026-01-15")), every: "month".into(), notice_days: 10, ..Contract::default() };
        assert_eq!(phone.next_renewal(today), Some(day("2026-10-15")));
        // The 31st stays the 31st after a short month.
        let last = Contract { renews: Some(day("2026-01-31")), every: "month".into(), ..Contract::default() };
        assert_eq!(last.next_renewal(today), Some(day("2026-10-31")));
        let once = Contract { renews: Some(day("2026-09-01")), ..Contract::default() };
        assert_eq!(once.next_renewal(today), None, "a date past, no term: nothing comes");
        assert_eq!(Contract::default().cancel_by(today), None);
    }

    #[test]
    fn kinds_guessed() {
        let words = crate::words::Words::builtin();
        let guess = |text: &str| Kind::guess(&words, text);
        assert_eq!(guess("Loyer octobre"), Some(Kind::Rent));
        assert_eq!(guess("EDF électricité"), Some(Kind::Energy));
        assert_eq!(guess("Free Mobile"), Some(Kind::Telecom));
        assert_eq!(guess("MAIF assurance habitation"), Some(Kind::Insurance));
        assert_eq!(guess("Mutuelle"), Some(Kind::Health));
        assert_eq!(guess("Abonnement annuel"), Some(Kind::Subscription));
        assert_eq!(guess("Exemple hébergement"), Some(Kind::Hosting));
        assert_eq!(guess("Boulangerie"), None);
        // A word inside another is not it: "redevance" is not RED.
        assert_eq!(guess("redevance"), None);
        // A brand of your own subscriptions, added in [words].
        assert_eq!(guess("Exemplestream Premium"), None);
        let config: crate::config::Config = toml::from_str("[words]\nlanguages = [\"fr\", \"en\"]\ncountries = [\"FR\"]\n[words.contracts.kinds.subscription]\nadd = [\"Exemplestream\"]\n").unwrap();
        assert_eq!(Kind::guess(&crate::words::Words::of(&config), "Exemplestream Premium"), Some(Kind::Subscription));
    }

    #[test]
    fn kept_and_read_again() {
        let root = std::env::temp_dir().join(format!("sioul-contracts-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let mut contracts = Contracts::load(&root).unwrap();
        let id = contracts.put(Contract { kind: Kind::Insurance, title: "Assurance habitation".into(), party: "MAIF".into(), reference: "12345".into(), renews: Some(day("2026-12-01")), every: "year".into(), notice_days: 30, cancel: "https://example.org/resilier".into(), covers: "Responsabilité civile, protection juridique".into(), ..Contract::default() });
        assert_eq!(id, "assurance-habitation");
        contracts.save().unwrap();
        let again = Contracts::load(&root).unwrap();
        assert_eq!(again.list, contracts.list);
        let _ = std::fs::remove_dir_all(&root);
    }
    #[test]
    fn the_letter_reads_as_a_letter() {
        for (language, opening) in [("fr", "Madame, Monsieur,\n\nPar la présente"), ("en", "Dear Sir or Madam,\n\nI hereby")] {
            let tr = crate::i18n::Translator::new(language);
            let mut args = crate::i18n::args();
            args.set("title", "Assurance habitation");
            args.set("party", "MAIF");
            args.set("reference", "n° 123");
            let body = tr.text("contract-letter-body", Some(&args));
            assert!(body.starts_with(opening), "{body}");
            assert!(body.contains("Assurance habitation") && body.contains("n° 123") && !body.contains("    "), "{body}");
        }
    }

}

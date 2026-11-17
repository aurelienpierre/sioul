// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! A country's rules as data (docs/projects.md, "Invoices"): one file per
//! country in `data/invoice-presets/`, built into Sioul. A preset says its
//! labels in each language, the legal forms, income regimes and VAT schemes
//! it offers, the identifiers its invoices carry and which are required to
//! issue, its dated rates, its legal lines with their sources (in the file's
//! comments), its late-payment terms, its thresholds and the contribution
//! rates of its declarations. Code reads them; no country has a code path of
//! its own. Legal form, income regime and VAT scheme are three settings of
//! their own: a French EI micro-entrepreneur may be liable for VAT, a company
//! under the franchise. Spain and Italy are listed as not supported, with
//! the reason, rather than offered as a template that would be illegal there.

use jiff::civil::Date;
use serde::Deserialize;
use std::collections::BTreeMap;

/// The presets' files, built in.
const FILES: &[(&str, &str)] = &[
    ("fr", include_str!("../../data/invoice-presets/fr.toml")),
    ("de", include_str!("../../data/invoice-presets/de.toml")),
    ("be", include_str!("../../data/invoice-presets/be.toml")),
    ("uk", include_str!("../../data/invoice-presets/uk.toml")),
    ("ch", include_str!("../../data/invoice-presets/ch.toml")),
    ("elsewhere", include_str!("../../data/invoice-presets/elsewhere.toml")),
    ("es", include_str!("../../data/invoice-presets/es.toml")),
    ("it", include_str!("../../data/invoice-presets/it.toml")),
];

/// The preset of an issuer who chose none: no legal claims, every line theirs.
pub const ELSEWHERE: &str = "elsewhere";

/// A text in each language, English when one lacks.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(transparent)]
pub struct Words(pub BTreeMap<String, String>);

impl Words {
    /// The text in `language` ("fr", "en"), else in English, else the first.
    pub fn get(&self, language: &str) -> &str {
        self.0.get(language).or_else(|| self.0.get("en")).or_else(|| self.0.values().next()).map_or("", String::as_str)
    }

    pub fn is_empty(&self) -> bool {
        self.0.values().all(|t| t.trim().is_empty())
    }
}

/// A value from a day on: a rate, a threshold, a mention.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Dated<T> {
    pub from: Date,
    #[serde(flatten)]
    pub value: T,
}

/// The value in force on `day` among dated ones: the latest from a day not after it.
pub fn on<T>(list: &[Dated<T>], day: Date) -> Option<&T> {
    list.iter().filter(|d| d.from <= day).max_by_key(|d| d.from).map(|d| &d.value)
}

/// A legal form the preset offers.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct LegalForm {
    pub id: String,
    pub name: Words,
    /// Its name is followed by a mark the law asks for: « EI » for a French
    /// entrepreneur individuel (C. com. R526-27), whatever the VAT.
    #[serde(default)]
    pub mark: String,
    /// A company: its capital is printed (`LegalLine::form`).
    #[serde(default)]
    pub company: bool,
}

/// An income regime for the books: "micro" (URSSAF declarations of turnover
/// received, the micro ceilings watched), "reel".
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Regime {
    pub id: String,
    pub name: Words,
}

/// A kind of identifier an invoice may carry.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct IdKind {
    pub kind: String,
    pub name: Words,
    /// How its value is checked (`ids::Check::named`): "siren", "siret", "vat-fr", else none.
    #[serde(default)]
    pub check: String,
    /// Printed before the value: "RCS" before a city. Empty: its name.
    #[serde(default)]
    pub before: Words,
}

/// Who a rule speaks of.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Who {
    #[default]
    Issuer,
    Client,
}

/// How much a rule weighs before issue.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    /// Issue waits for it.
    #[default]
    Required,
    /// Said, issue goes on.
    Recommended,
}

/// What an invoice must carry, said before issue (`life::checks`): one of
/// some identifiers, or a field, of the issuer or of the client, for some
/// clients, between two days.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Require {
    #[serde(default)]
    pub who: Who,
    /// One of these identifiers' kinds.
    #[serde(default)]
    pub any: Vec<String>,
    /// Or a field: "name", "address", "mention", "delivery", "city".
    #[serde(default)]
    pub field: String,
    /// The clients it applies to: "business", "consumer", "public"; empty: all.
    #[serde(default)]
    pub client: Vec<String>,
    /// Where those clients are, from the issuer's country: "domestic", "eu"
    /// (another member state), "abroad" (outside the EU); empty: anywhere.
    #[serde(default)]
    pub zone: Vec<String>,
    /// Lines of which nature: "goods", "service"; empty: any.
    #[serde(default)]
    pub nature: Vec<String>,
    /// Under which VAT schemes: "none", "franchise", "liable"; empty: any.
    #[serde(default)]
    pub scheme: Vec<String>,
    /// For which legal forms; "company" for any company form; empty: all.
    #[serde(default)]
    pub form: Vec<String>,
    /// When a line of the invoice has one of these VAT categories ("AE",
    /// "K"); empty: whatever the lines.
    #[serde(default)]
    pub category: Vec<String>,
    /// For an issuer with one of these activities ("goods-bic"); empty: any.
    #[serde(default)]
    pub activity: Vec<String>,
    #[serde(default)]
    pub level: Level,
    #[serde(default)]
    pub from: Option<Date>,
    #[serde(default)]
    pub until: Option<Date>,
    /// The sentence said when it is missing.
    pub why: Words,
}

/// A VAT rate a preset offers, in basis points (2000 = 20 %).
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Rate {
    pub rate: i64,
    pub name: Words,
}

/// The VAT schemes a preset offers.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct Tax {
    /// What the person may choose: "none" (not registered), "franchise" (a
    /// small-business exemption), "liable"; the first is the default.
    #[serde(default)]
    pub schemes: Vec<String>,
    /// The franchise: EN 16931's category, its reason code and the mention,
    /// dated, editable by the person.
    #[serde(default)]
    pub franchise: Option<Exempt>,
    /// An exempt line under VAT: its category; the line says its exemption.
    #[serde(default)]
    pub exempt: Option<Exempt>,
    /// Liable for VAT: the rates, dated.
    #[serde(default)]
    pub rates: Vec<Dated<RateList>>,
    /// Where a line's VAT is due, by the scheme, the client's zone and kind,
    /// and what the line sells (`life::placed`): the reverse charge, an
    /// intra-EU delivery, an export, outside the scope; the first that
    /// applies places the line.
    #[serde(default)]
    pub place: Vec<Place>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct RateList {
    pub rates: Vec<Rate>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct Exempt {
    pub category: String,
    #[serde(default)]
    pub reason_code: String,
    /// The mention, dated; empty when the person must write it.
    #[serde(default)]
    pub mention: Vec<Dated<Mention>>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Mention {
    pub text: String,
}

/// Where a line's VAT is due: under these schemes, for clients of these
/// kinds in these zones, for lines of this nature; the category the line
/// takes, EN 16931's reason code, and the words printed for it. Without
/// words (the research gave none), the line prints no mention and a
/// required check says `why` until the line says its own.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct Place {
    pub id: String,
    #[serde(default)]
    pub scheme: Vec<String>,
    #[serde(default)]
    pub zone: Vec<String>,
    #[serde(default)]
    pub client: Vec<String>,
    #[serde(default)]
    pub nature: Vec<String>,
    pub category: String,
    #[serde(default)]
    pub reason_code: String,
    #[serde(default)]
    pub mention: Words,
    #[serde(default)]
    pub why: Words,
}

/// A line printed on invoices, with its EN 16931 subject code (UNTDID 4451).
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct LegalLine {
    pub id: String,
    #[serde(default)]
    pub subject: String,
    /// The clients it is printed for; empty: all.
    #[serde(default)]
    pub client: Vec<String>,
    /// Printed only with this option of the issuer's ("debits"); empty: always.
    #[serde(default)]
    pub option: String,
    /// Printed only under these VAT schemes; empty: any.
    #[serde(default)]
    pub scheme: Vec<String>,
    /// Printed only for these legal forms ("company": any company form); empty: all.
    #[serde(default)]
    pub form: Vec<String>,
    /// Its text; `{ $rate }` is the late-payment rate kept with the invoice.
    pub text: Words,
}

/// Late-payment penalties: a rate from the central bank's plus points, the
/// reference rate taken on 1 January and 1 July, a floor; each invoice keeps
/// the rate it printed.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct Penalty {
    /// Added to the reference rate, in basis points.
    pub points: i64,
    /// The reference rate (the ECB's main refinancing operations), dated, in basis points.
    #[serde(default)]
    pub reference: Vec<Dated<Bp>>,
    /// A contractual rate may not be lower than this many times the legal interest rate.
    #[serde(default)]
    pub floor_times: i64,
    /// The legal interest rate for professional creditors, dated, in basis points.
    #[serde(default)]
    pub legal: Vec<Dated<Bp>>,
    /// Said after the rate when it is the default: "taux BCE + 10 points".
    #[serde(default)]
    pub default_note: Words,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Bp {
    pub rate: i64,
}

/// A turnover threshold, in cents of the preset's currency, from a day.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Threshold {
    pub id: String,
    /// "micro" (the regime's ceiling), "franchise" (the VAT franchise's base
    /// threshold), "franchise-higher" (its higher one), "registration".
    pub kind: String,
    /// The activities it counts ("goods-bic", "services-bic", "bnc", "cipav"); empty: all.
    #[serde(default)]
    pub activity: Vec<String>,
    /// Watched only under this VAT scheme ("franchise") or income regime ("micro"); empty: always.
    #[serde(default)]
    pub scheme: String,
    #[serde(default)]
    pub regime: String,
    /// What it counts: "all", the whole turnover (France's goods thresholds,
    /// which a mixed activity keeps in all); empty, only what its activities
    /// sell (their nature: the services' part for services).
    #[serde(default)]
    pub counts: String,
    /// Prorated to the days of activity in the first year (ceiling × days / 365).
    #[serde(default)]
    pub prorated: bool,
    /// Crossed, invoices to businesses since that day are corrected (France's higher thresholds).
    #[serde(default)]
    pub corrective: bool,
    pub amount: i64,
    pub from: Date,
}

/// The declarations of turnover a regime asks (France's micro regime, to
/// URSSAF): the turnover received in each period, one amount per activity.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct DeclarationRule {
    /// The income regime it is for ("micro").
    pub regime: String,
    /// The periods one may choose, the first by default: "quarter", "month".
    pub periods: Vec<String>,
    /// Due the last day of the month this many months after the period's,
    /// moved to the next working day.
    pub months_after: u32,
    /// The first one at least this many days after the start of activity.
    #[serde(default)]
    pub first_after_days: i64,
}

/// A declaration of services to businesses in other member states (France's
/// DES): the lines of these categories and natures, month by month, due the
/// n-th working day of the following month.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct DesRule {
    pub category: Vec<String>,
    #[serde(default)]
    pub nature: Vec<String>,
    pub working_day: u32,
}

/// A contribution rate on turnover received, in basis points, from a day.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Contribution {
    /// "social", "vl" (versement libératoire), "cfp" (formation professionnelle).
    pub kind: String,
    /// The activity ("goods-bic", "services-bic", "bnc", "cipav") or, for
    /// the CFP, the trade ("commerce", "craft", "liberal").
    pub activity: String,
    /// Under which income regime ("micro"); empty: any.
    #[serde(default)]
    pub regime: String,
    pub rate: i64,
    pub from: Date,
}

/// An activity for the books.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Activity {
    pub id: String,
    pub name: Words,
    /// "goods" or "service": the nature of its lines.
    pub nature: String,
}

/// One preset.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct Preset {
    pub id: String,
    /// ISO 3166-1 code; empty for "elsewhere".
    #[serde(default)]
    pub country: String,
    pub name: Words,
    /// False for a country whose invoices Sioul cannot make lawfully (Spain, Italy).
    #[serde(default = "yes")]
    pub supported: bool,
    /// Why not, when not.
    #[serde(default)]
    pub why_not: Words,
    /// What Sioul does not do under this preset, said in Settings and the guide.
    #[serde(default)]
    pub limits: Words,
    #[serde(default)]
    pub currency: String,
    #[serde(default)]
    pub legal_form: Vec<LegalForm>,
    #[serde(default)]
    pub regime: Vec<Regime>,
    #[serde(default)]
    pub identifier: Vec<IdKind>,
    #[serde(default)]
    pub require: Vec<Require>,
    #[serde(default)]
    pub tax: Tax,
    #[serde(default)]
    pub line: Vec<LegalLine>,
    #[serde(default)]
    pub penalty: Option<Penalty>,
    /// Payment terms by default, in days, and their legal ceiling.
    #[serde(default)]
    pub terms_days: Option<u32>,
    #[serde(default)]
    pub terms_max_days: Option<u32>,
    #[serde(default)]
    pub activity: Vec<Activity>,
    #[serde(default)]
    pub threshold: Vec<Threshold>,
    #[serde(default)]
    pub contribution: Vec<Contribution>,
    /// The declarations of turnover, when the country asks them (`books`).
    #[serde(default)]
    pub declaration: Option<DeclarationRule>,
    /// The declaration of services to businesses in other member states (`books::des`).
    #[serde(default)]
    pub des: Option<DesRule>,
}

fn yes() -> bool {
    true
}

/// Every preset, in the order Settings lists them.
pub fn all() -> &'static [Preset] {
    static ALL: std::sync::OnceLock<Vec<Preset>> = std::sync::OnceLock::new();
    ALL.get_or_init(|| FILES.iter().filter_map(|(_, text)| toml::from_str::<Preset>(text).ok()).collect())
}

/// A preset by its id; "elsewhere" for any id Sioul does not know.
pub fn get(id: &str) -> &'static Preset {
    let all = all();
    all.iter().find(|p| p.id == id).or_else(|| all.iter().find(|p| p.id == ELSEWHERE)).expect("the preset elsewhere is built in")
}

/// The member states of the European Union (ISO 3166-1), for the zone of a
/// client: another member state's business is billed by the EU's rules.
pub const EU: [&str; 27] = ["AT", "BE", "BG", "CY", "CZ", "DE", "DK", "EE", "ES", "FI", "FR", "GR", "HR", "HU", "IE", "IT", "LT", "LU", "LV", "MT", "NL", "PL", "PT", "RO", "SE", "SI", "SK"];

/// Country names, as an address's last line writes them, in English,
/// French and the country's own language, for the countries an invoice
/// most often goes to: their ISO 3166-1 code.
const COUNTRIES: &[(&str, &[&str])] = &[
    ("AT", &["austria", "autriche", "osterreich"]),
    ("BE", &["belgium", "belgique", "belgie", "belgien"]),
    ("BG", &["bulgaria", "bulgarie"]),
    ("CY", &["cyprus", "chypre"]),
    ("CZ", &["czechia", "czech republic", "tchequie", "republique tcheque", "cesko"]),
    ("DE", &["germany", "allemagne", "deutschland"]),
    ("DK", &["denmark", "danemark", "danmark"]),
    ("EE", &["estonia", "estonie", "eesti"]),
    ("ES", &["spain", "espagne", "espana"]),
    ("FI", &["finland", "finlande", "suomi"]),
    ("FR", &["france"]),
    ("GR", &["greece", "grece", "hellas"]),
    ("HR", &["croatia", "croatie", "hrvatska"]),
    ("HU", &["hungary", "hongrie", "magyarorszag"]),
    ("IE", &["ireland", "irlande", "eire"]),
    ("IT", &["italy", "italie", "italia"]),
    ("LT", &["lithuania", "lituanie", "lietuva"]),
    ("LU", &["luxembourg", "luxemburg"]),
    ("LV", &["latvia", "lettonie", "latvija"]),
    ("MT", &["malta", "malte"]),
    ("NL", &["netherlands", "the netherlands", "pays-bas", "pays bas", "nederland", "holland", "hollande"]),
    ("PL", &["poland", "pologne", "polska"]),
    ("PT", &["portugal"]),
    ("RO", &["romania", "roumanie"]),
    ("SE", &["sweden", "suede", "sverige"]),
    ("SI", &["slovenia", "slovenie", "slovenija"]),
    ("SK", &["slovakia", "slovaquie", "slovensko"]),
    ("GB", &["united kingdom", "royaume-uni", "royaume uni", "uk", "great britain", "grande-bretagne", "england", "angleterre", "scotland", "ecosse", "wales"]),
    ("CH", &["switzerland", "suisse", "schweiz", "svizzera"]),
    ("NO", &["norway", "norvege", "norge"]),
    ("IS", &["iceland", "islande"]),
    ("LI", &["liechtenstein"]),
    ("MC", &["monaco"]),
    ("AD", &["andorra", "andorre"]),
    ("US", &["united states", "united states of america", "usa", "etats-unis", "etats unis"]),
    ("CA", &["canada"]),
];

/// The ISO 3166-1 code of a country an address's line names ("Belgique",
/// "Deutschland", "BE"); None for a line that names none Sioul knows.
pub fn country_code(line: &str) -> Option<&'static str> {
    let folded: String = crate::text::fold(line.trim()).into_iter().collect::<String>().trim_matches(|c: char| c == '.' || c.is_whitespace()).to_string();
    if folded.is_empty() {
        return None;
    }
    COUNTRIES.iter().find(|(code, names)| folded.eq_ignore_ascii_case(code) || names.iter().any(|n| *n == folded)).map(|(code, _)| *code)
}

/// Where a client in `client` is from an issuer in `issuer`: "domestic",
/// "eu" (another member state, the issuer's in the EU too), "abroad".
pub fn zone(issuer: &str, client: &str) -> &'static str {
    let (issuer, client) = (issuer.trim().to_uppercase(), client.trim().to_uppercase());
    // Greece writes "EL" in VAT numbers, "GR" in addresses.
    let eu = |c: &str| EU.contains(&c) || c == "EL";
    if client.is_empty() || client == issuer {
        "domestic"
    } else if eu(&issuer) && eu(&client) {
        "eu"
    } else {
        "abroad"
    }
}

impl Preset {
    /// The identifier kind `kind`, when the preset has it.
    pub fn id_kind(&self, kind: &str) -> Option<&IdKind> {
        self.identifier.iter().find(|k| k.kind == kind)
    }

    /// The franchise's mention in force on `day`; "" when the person must write it.
    pub fn mention_on(&self, day: Date) -> &str {
        self.tax.franchise.as_ref().and_then(|e| on(&e.mention, day)).map_or("", |m| m.text.as_str())
    }

    /// The VAT rates in force on `day`.
    pub fn rates_on(&self, day: Date) -> &[Rate] {
        on(&self.tax.rates, day).map_or(&[], |r| r.rates.as_slice())
    }

    /// The default late-payment rate on `day`, in basis points: the
    /// reference rate in force on 1 January for an invoice of the first
    /// half-year, on 1 July for the second, plus the points.
    pub fn penalty_on(&self, day: Date) -> Option<i64> {
        let penalty = self.penalty.as_ref()?;
        let half = Date::new(day.year(), if day.month() >= 7 { 7 } else { 1 }, 1).ok()?;
        Some(on(&penalty.reference, half)?.rate + penalty.points)
    }

    /// The lowest rate a contract may set on `day`, in basis points.
    pub fn penalty_floor_on(&self, day: Date) -> Option<i64> {
        let penalty = self.penalty.as_ref()?;
        (penalty.floor_times > 0).then(|| on(&penalty.legal, day).map(|l| l.rate * penalty.floor_times)).flatten()
    }

    /// The income regime `id`, when the preset has it.
    pub fn regime_of(&self, id: &str) -> Option<&Regime> {
        self.regime.iter().find(|r| r.id == id)
    }

    /// The thresholds of `kind` in force on `day`, by id.
    pub fn thresholds_on(&self, kind: &str, day: Date) -> Vec<&Threshold> {
        let mut by: BTreeMap<&str, &Threshold> = BTreeMap::new();
        for t in self.threshold.iter().filter(|t| t.kind == kind && t.from <= day) {
            if by.get(t.id.as_str()).is_none_or(|kept| kept.from < t.from) {
                by.insert(&t.id, t);
            }
        }
        by.into_values().collect()
    }

    /// The contribution rate of `kind` for `activity` on `day`, in basis points.
    pub fn contribution_on(&self, kind: &str, activity: &str, day: Date) -> Option<i64> {
        self.contribution.iter().filter(|c| c.kind == kind && c.activity == activity && c.from <= day).max_by_key(|c| c.from).map(|c| c.rate)
    }

    /// The legal form `id`, when the preset has it.
    pub fn form(&self, id: &str) -> Option<&LegalForm> {
        self.legal_form.iter().find(|f| f.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every file reads, under its own id, and says what each of its rules
    /// names: identifiers known, levels, schemes it offers.
    #[test]
    fn every_preset_reads() {
        for (id, text) in FILES {
            let preset: Preset = toml::from_str(text).unwrap_or_else(|e| panic!("{id}: {e}"));
            assert_eq!(&preset.id, id);
            assert!(!preset.name.get("en").is_empty() && !preset.name.get("fr").is_empty(), "{id}: named in both languages");
            if !preset.supported {
                assert!(!preset.why_not.get("en").is_empty() && !preset.why_not.get("fr").is_empty(), "{id}: says why not");
                continue;
            }
            for rule in &preset.require {
                for kind in &rule.any {
                    assert!(preset.id_kind(kind).is_some(), "{id}: a rule names {kind}, which it has not");
                }
                assert!(!rule.any.is_empty() || !rule.field.is_empty(), "{id}: a rule names nothing");
                assert!(!rule.why.get("en").is_empty() && !rule.why.get("fr").is_empty(), "{id}: a rule says why in both languages");
            }
            for scheme in preset.tax.schemes.iter().chain(preset.require.iter().flat_map(|r| &r.scheme)).chain(preset.line.iter().flat_map(|l| &l.scheme)) {
                assert!(matches!(scheme.as_str(), "none" | "franchise" | "liable"), "{id}: {scheme}");
            }
            for form in preset.require.iter().flat_map(|r| &r.form).chain(preset.line.iter().flat_map(|l| &l.form)) {
                assert!(form == "company" || preset.form(form).is_some(), "{id}: names the form {form}, which it has not");
            }
            for place in &preset.tax.place {
                assert!(place.scheme.iter().all(|s| preset.tax.schemes.contains(s)), "{id}: {} names a scheme it does not offer", place.id);
                assert!(matches!(place.category.as_str(), "S" | "Z" | "E" | "AE" | "K" | "G" | "O"), "{id}: {}", place.id);
                assert!(place.zone.iter().all(|z| matches!(z.as_str(), "domestic" | "eu" | "abroad")), "{id}: {}", place.id);
                // Without the research's words, the rule says why the line must say its own.
                assert!(!place.mention.is_empty() || (!place.why.get("en").is_empty() && !place.why.get("fr").is_empty()), "{id}: {} has neither words nor a why", place.id);
            }
            if preset.tax.schemes.iter().any(|s| s == "franchise") {
                assert!(preset.tax.franchise.is_some(), "{id}: a franchise without its category");
            }
            if preset.tax.schemes.iter().any(|s| s == "liable") && preset.id != ELSEWHERE {
                assert!(!preset.rates_on("2026-11-16".parse().unwrap()).is_empty(), "{id}: VAT without rates");
            }
            for line in &preset.line {
                assert!(!line.text.get("en").is_empty() && !line.text.get("fr").is_empty(), "{id}: line {} in both languages", line.id);
            }
        }
        assert_eq!(all().len(), FILES.len());
        assert_eq!(get("nowhere").id, ELSEWHERE);
    }

    /// The French figures of 2026, as the research found them (research-fr.md §7–8).
    #[test]
    fn french_figures_of_2026() {
        let micro = get("fr");
        let day: Date = "2026-11-16".parse().unwrap();
        assert_eq!(micro.mention_on(day), "TVA non applicable, article 293 B du CGI");
        let amount = |kind: &str, id: &str| micro.thresholds_on(kind, day).into_iter().find(|t| t.id == id).map(|t| t.amount);
        assert_eq!(amount("micro", "micro-goods"), Some(20_310_000));
        assert_eq!(amount("micro", "micro-services"), Some(8_360_000));
        assert_eq!(amount("franchise", "franchise-services"), Some(3_750_000));
        assert_eq!(amount("franchise-higher", "franchise-services-higher"), Some(4_125_000));
        assert_eq!(amount("franchise", "franchise-goods"), Some(8_500_000));
        assert_eq!(amount("franchise-higher", "franchise-goods-higher"), Some(9_350_000));
        assert_eq!(micro.contribution_on("social", "bnc", day), Some(2560));
        assert_eq!(micro.contribution_on("social", "goods-bic", day), Some(1230));
        assert_eq!(micro.contribution_on("social", "services-bic", day), Some(2120));
        assert_eq!(micro.contribution_on("social", "cipav", day), Some(2320));
        assert_eq!(micro.contribution_on("vl", "services-bic", day), Some(170));
        // ECB + 10 points: 2.15 % on 1 January 2026, 2.40 % on 1 July 2026.
        assert_eq!(micro.penalty_on("2026-03-02".parse().unwrap()), Some(1215));
        assert_eq!(micro.penalty_on(day), Some(1240));
        assert_eq!(micro.penalty_floor_on(day), Some(825));
        assert_eq!(zone("FR", "FR"), "domestic");
        assert_eq!(zone("FR", "be"), "eu");
        assert_eq!(zone("FR", "CH"), "abroad");
        assert_eq!(zone("GB", "FR"), "abroad");
        assert_eq!((country_code("Belgique"), country_code(" Deutschland "), country_code("BE"), country_code("Royaume-Uni"), country_code("Narnia")), (Some("BE"), Some("DE"), Some("BE"), Some("GB"), None));
    }
}

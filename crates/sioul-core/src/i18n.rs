// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Every sentence Sioul shows, in your language.
//!
//! The core decides and returns facts: lanes, reasons, counts. This module
//! turns them into sentences with Project Fluent, which handles plurals and
//! agreement per language ("une lettre est arrivée", "deux lettres sont
//! arrivées") where gettext's plural forms alone cannot. Each language is one
//! file, `locales/<language>/sioul.ftl`; English stands in for any message a
//! translation lacks (docs/i18n.md).

use crate::budget::{BudgetStatus, Period, ReserveStatus, Verdict};
use crate::cases::{CaseStore, RouteField, RouteMatch};
use crate::money::Money;
use crate::codes::CodeKind;
use crate::porch::{Reason, Summary};
use crate::trust::{Proof, Trust};
use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::{FluentArgs, FluentResource};
use jiff::Zoned;
use jiff::civil::Date;
use unic_langid::LanguageIdentifier;

/// The languages Sioul speaks, with their translations, built into the program.
pub const LANGUAGES: &[(&str, &str)] = &[
    ("en", include_str!("../locales/en/sioul.ftl")),
    ("fr", include_str!("../locales/fr/sioul.ftl")),
];

type Bundle = FluentBundle<FluentResource>;

/// Empty arguments, for callers that do not depend on Fluent themselves.
pub fn args() -> FluentArgs<'static> {
    FluentArgs::new()
}

/// Sentences in one language.
pub struct Translator {
    language: String,
    bundle: Bundle,
    fallback: Bundle,
}

impl Translator {
    /// A translator for a language code ("fr", "fr_FR.UTF-8", "en-GB"); English if Sioul lacks it.
    pub fn new(requested: &str) -> Translator {
        let wanted = base_language(requested);
        let language = if LANGUAGES.iter().any(|(l, _)| *l == wanted) { wanted } else { "en".to_string() };
        Translator { bundle: bundle(&language), fallback: bundle("en"), language }
    }

    /// The language actually used.
    pub fn language(&self) -> &str {
        &self.language
    }

    /// One message with its arguments; the English one if this language lacks it.
    pub fn text(&self, id: &str, args: Option<&FluentArgs>) -> String {
        format(&self.bundle, id, None, args)
            .or_else(|| format(&self.fallback, id, None, args))
            .unwrap_or_else(|| id.to_string())
    }

    fn attribute(&self, id: &str, attribute: &str) -> Option<String> {
        format(&self.bundle, id, Some(attribute), None).or_else(|| format(&self.fallback, id, Some(attribute), None))
    }

    /// A number in words: "two", "Two"; in French also "une", "Une". Digits beyond twelve.
    pub fn count(&self, n: usize, capital: bool, feminine: bool) -> String {
        let id = format!("count-{n}");
        let wanted: &[&str] = match (capital, feminine) {
            (true, true) => &["fcap", "cap"],
            (true, false) => &["cap"],
            (false, true) => &["f"],
            (false, false) => &[],
        };
        wanted
            .iter()
            .find_map(|a| format(&self.bundle, &id, Some(a), None))
            .or_else(|| format(&self.bundle, &id, None, None))
            .unwrap_or_else(|| n.to_string())
    }

    /// The arguments every counted sentence receives: `n`, `count`, `Count`, `countf`, `Countf`.
    pub fn counted(&self, n: usize) -> FluentArgs<'static> {
        let mut args = FluentArgs::new();
        args.set("n", n);
        args.set("count", self.count(n, false, false));
        args.set("Count", self.count(n, true, false));
        args.set("countf", self.count(n, false, true));
        args.set("Countf", self.count(n, true, true));
        args
    }

    /// What came, in a few calm sentences.
    pub fn summary(&self, s: &Summary, cases: Option<&CaseStore>) -> String {
        if s.total == 0 {
            let mut sentences = vec![self.text("summary-nothing", None)];
            self.push_counted(&mut sentences, "summary-low", s.low);
            return sentences.join(" ");
        }
        let mut sentences = vec![self.text("summary-total", Some(&self.counted(s.total)))];
        self.push_counted(&mut sentences, "summary-codes", s.codes);
        if !s.cases.is_empty() {
            sentences.push(self.case_sentence(s, cases));
        }
        self.push_counted(&mut sentences, "summary-people", s.people);
        self.push_counted(&mut sentences, "summary-screener", s.screener);
        self.push_counted(&mut sentences, "summary-filed", s.filed);
        self.push_counted(&mut sentences, "summary-set-aside", s.set_aside);
        self.push_counted(&mut sentences, "summary-low", s.low);
        sentences.join(" ")
    }

    fn push_counted(&self, sentences: &mut Vec<String>, id: &str, n: usize) {
        if n > 0 {
            sentences.push(self.text(id, Some(&self.counted(n))));
        }
    }

    /// "Three belong to cases: Taxes (two), Health cover (one)."
    fn case_sentence(&self, s: &Summary, cases: Option<&CaseStore>) -> String {
        let n: usize = s.cases.iter().map(|(_, k)| k).sum();
        let several = s.cases.len() > 1;
        let parts: Vec<String> = s
            .cases
            .iter()
            .map(|(id, k)| {
                let title = case_title(id, cases);
                if !several {
                    return title;
                }
                let mut args = self.counted(*k);
                args.set("title", title);
                self.text("summary-case-part", Some(&args))
            })
            .collect();
        let mut args = self.counted(n);
        args.set("k", s.cases.len());
        args.set("cases", parts.join(", "));
        self.text("summary-cases", Some(&args))
    }

    /// One reason, as a phrase.
    pub fn reason(&self, reason: &Reason, cases: Option<&CaseStore>) -> String {
        match reason {
            Reason::Trust(proof) => self.text(proof_id(*proof), None),
            Reason::Forged => self.text("reason-forged", None),
            Reason::Spam { source, score } => self.spam_reason(source, *score),
            // Your own filter's word, never why nor how sure (the owner's decision: no "why").
            Reason::LearnedSpam { .. } => self.text("reason-learned-spam", None),
            Reason::Unsure { .. } => self.text("reason-unsure", None),
            Reason::LearnedHam { .. } => self.text("reason-learned-ham", None),
            Reason::MovedToJunk => self.text("reason-moved-to-junk", None),
            Reason::Blocked => self.text("reason-blocked", None),
            Reason::Impersonation { brand, domain } => {
                let mut args = FluentArgs::new();
                args.set("brand", brand.clone());
                args.set("domain", domain.clone());
                self.text("reason-impersonation", Some(&args))
            }
            Reason::ExpiresSoon(kind) => self.text("reason-expires-soon", Some(&self.kind_args(*kind))),
            Reason::UnverifiedCode(kind) => self.text("reason-unverified-code", Some(&self.kind_args(*kind))),
            Reason::Case { case_id, matched } => {
                let mut args = FluentArgs::new();
                args.set("title", case_title(case_id, cases));
                args.set("why", matched.iter().map(|m| self.route_match(m)).collect::<Vec<_>>().join(", "));
                self.text("reason-case", Some(&args))
            }
            Reason::Hostile => self.text("reason-hostile", None),
            Reason::Public(topic) => {
                let mut args = FluentArgs::new();
                args.set("topic", self.topic(*topic));
                self.text("reason-public", Some(&args))
            }
            Reason::Newsletter => self.text("reason-newsletter", None),
            Reason::Automatic => self.text("reason-automatic", None),
            Reason::FirstMessage => self.text("reason-first-message", None),
            Reason::NotAuthenticated => self.text("reason-not-authenticated", None),
            Reason::KnownPerson => self.text("reason-known", None),
            Reason::FromYourself => self.text("reason-from-yourself", None),
            Reason::LowPriority => self.text("reason-low-priority", None),
        }
    }

    fn spam_reason(&self, source: &str, score: Option<f32>) -> String {
        let mut args = FluentArgs::new();
        args.set("source", source.to_string());
        let id = score.map_or("reason-spam", |s| {
            args.set("score", self.decimal(s));
            "reason-spam-score"
        });
        self.text(id, Some(&args))
    }

    /// A decimal with this language's separator: "9.1", "9,1".
    pub fn decimal(&self, value: f32) -> String {
        format!("{value:.1}").replace('.', &self.text("decimal-separator", None))
    }

    fn route_match(&self, m: &RouteMatch) -> String {
        let id = match m.field {
            RouteField::SenderDomain => "route-sender-domain",
            RouteField::Sender => "route-sender",
            RouteField::Subject => "route-subject",
            RouteField::Text => "route-text",
            RouteField::Attachment => "route-attachment",
            RouteField::Thread => "route-thread",
        };
        let mut args = FluentArgs::new();
        args.set("value", m.value.clone());
        self.text(id, Some(&args))
    }

    /// `kind`, `Kind` (capitalised) and `akind` (with its article: "a code", "un code").
    fn kind_args(&self, kind: CodeKind) -> FluentArgs<'static> {
        let id = kind_id(kind);
        let mut args = FluentArgs::new();
        args.set("kind", self.text(id, None));
        args.set("Kind", self.attribute(id, "cap").unwrap_or_else(|| self.text(id, None)));
        args.set("akind", self.attribute(id, "a").unwrap_or_else(|| self.text(id, None)));
        args
    }

    /// "Code from La Banque Exemple (verified): 482913, valid about 10 minutes".
    pub fn right_now(&self, kind: CodeKind, sender: &str, trust: Trust, code: Option<&str>, minutes: Option<u32>) -> String {
        let mut args = self.kind_args(kind);
        args.set("sender", sender.to_string());
        args.set("trust", self.trust(trust));
        let id = code.map_or("right-now-item", |c| {
            args.set("code", c.to_string());
            "right-now-item-code"
        });
        let line = self.text(id, Some(&args));
        minutes.map_or(line.clone(), |m| format!("{line}, {}", self.validity(m)))
    }

    /// "valid about 10 minutes", "valid about 2 hours", "valid about 2 days".
    pub fn validity(&self, minutes: u32) -> String {
        let mut args = FluentArgs::new();
        let id = if minutes < 120 {
            args.set("minutes", minutes);
            "right-now-valid"
        } else if minutes < 48 * 60 {
            args.set("hours", minutes.div_ceil(60));
            "right-now-valid-hours"
        } else {
            args.set("days", minutes.div_ceil(24 * 60));
            "right-now-valid-days"
        };
        self.text(id, Some(&args))
    }

    /// "verified", "not verified", "forged".
    pub fn trust(&self, trust: Trust) -> String {
        let id = match trust {
            Trust::Verified => "trust-verified",
            Trust::Unverified => "trust-unverified",
            Trust::Forged => "trust-forged",
        };
        self.text(id, None)
    }

    /// "on Tuesday 6 October at 10:00", "le mardi 6 octobre à 10:00".
    pub fn when(&self, at: &Zoned) -> String {
        let mut args = FluentArgs::new();
        args.set("date", self.date(at, false));
        self.text("when-on", Some(&args))
    }

    /// A date, long ("Tuesday 6 October at 10:00") or short ("Tue 6 Oct 10:00").
    pub fn date(&self, at: &Zoned, short: bool) -> String {
        let weekday = format!("weekday-{}", at.weekday().to_monday_one_offset());
        let month = format!("month-{}", at.month());
        let name = |id: &str| if short { self.attribute(id, "short").unwrap_or_else(|| self.text(id, None)) } else { self.text(id, None) };
        let mut args = FluentArgs::new();
        args.set("weekday", name(&weekday));
        args.set("day", at.day());
        args.set("month", name(&month));
        args.set("time", at.strftime("%H:%M").to_string());
        self.text(if short { "date-short" } else { "date-long" }, Some(&args))
    }
}

impl Translator {
    /// "€1,234.56", "1 234,56 €"; whole amounts without cents.
    pub fn money(&self, m: Money) -> String {
        let mut args = FluentArgs::new();
        args.set("value", self.digits(m));
        self.text(if m.is_negative() { "money-negative" } else { "money-positive" }, Some(&args))
    }

    /// An amount in another currency, signed, without symbol: "−20", "−1 234,50".
    pub fn number(&self, m: Money) -> String {
        let sign = if m.is_negative() { "−" } else { "" };
        format!("{sign}{}", self.digits(m))
    }

    /// The digits of an amount, grouped, without sign.
    fn digits(&self, m: Money) -> String {
        let (units, cents) = m.split();
        let grouped = group_thousands(units, &self.text("thousands-separator", None));
        if cents == 0 { grouped } else { format!("{grouped}{}{cents:02}", self.text("decimal-separator", None)) }
    }

    /// What a shielded message is about: "work", "a question".
    pub fn topic(&self, topic: crate::shield::Topic) -> String {
        let id = match topic {
            crate::shield::Topic::Work => "topic-work",
            crate::shield::Topic::Support => "topic-support",
            crate::shield::Topic::Press => "topic-press",
            crate::shield::Topic::Thanks => "topic-thanks",
            crate::shield::Topic::Donation => "topic-donation",
            crate::shield::Topic::Other => "topic-other",
        };
        self.text(id, None)
    }

    /// "Monday 5 October", "lundi 5 octobre".
    pub fn day(&self, d: Date) -> String {
        let mut args = FluentArgs::new();
        args.set("weekday", self.text(&format!("weekday-{}", d.weekday().to_monday_one_offset()), None));
        args.set("day", d.day());
        args.set("month", self.text(&format!("month-{}", d.month()), None));
        self.text("date-day", Some(&args))
    }

    /// "Monday 5 October", with its year when it is not `today`'s: "Monday 1 March 2027".
    pub fn day_in(&self, d: Date, today: Date) -> String {
        if d.year() == today.year() { self.day(d) } else { format!("{} {}", self.day(d), d.year()) }
    }

    /// "Mon", "lun.".
    pub fn weekday_short(&self, d: Date) -> String {
        let id = format!("weekday-{}", d.weekday().to_monday_one_offset());
        self.attribute(&id, "short").unwrap_or_else(|| self.text(&id, None))
    }

    /// "Jun", "juin"; "Jul", "juil.": a month's short name, as a graph's axis says it.
    pub fn month_short(&self, d: Date) -> String {
        let id = format!("month-{}", d.month());
        self.attribute(&id, "short").unwrap_or_else(|| self.text(&id, None))
    }

    /// "31 October", "31 octobre".
    pub fn day_month(&self, d: Date) -> String {
        let mut args = FluentArgs::new();
        args.set("day", d.day());
        args.set("month", self.text(&format!("month-{}", d.month()), None));
        self.text("date-day-month", Some(&args))
    }

    /// "October 2026", "octobre 2026".
    pub fn month_year(&self, d: Date) -> String {
        let mut args = FluentArgs::new();
        args.set("month", self.text(&format!("month-{}", d.month()), None));
        args.set("year", d.year().to_string());
        self.text("month-year", Some(&args))
    }

    /// One budget's period, in a sentence, and what a reserve does at its end.
    pub fn budget_lines(&self, title: &str, period: Period, s: &BudgetStatus, reserve_title: Option<&str>) -> Vec<String> {
        let end = self.day_month(s.end);
        let mut verdict_args = FluentArgs::new();
        verdict_args.set("gap", self.money(s.gap.abs()));
        let verdict = match s.verdict {
            Verdict::Better => "verdict-better",
            Verdict::AsPlanned => "verdict-as-planned",
            Verdict::Short => "verdict-short",
        };
        let mut args = FluentArgs::new();
        args.set("title", title.to_string());
        args.set("period", self.text(if period == Period::Month { "period-month" } else { "period-year" }, None));
        args.set("so_far", self.money(s.so_far));
        args.set("projected", self.money(s.projected));
        args.set("end", end.clone());
        args.set("target", self.money(s.target));
        args.set("verdict", self.text(verdict, Some(&verdict_args)));
        let mut lines = vec![self.text("budget-line", Some(&args))];
        if s.earmarked != Money::ZERO {
            let mut args = FluentArgs::new();
            args.set("amount", self.money(s.earmarked));
            lines.push(self.text("budget-earmarked", Some(&args)));
        }
        if s.opening != Money::ZERO {
            let mut args = FluentArgs::new();
            args.set("opening", self.money(s.opening));
            lines.push(self.text("budget-opening", Some(&args)));
        }
        if let (Some((_, amount)), Some(reserve)) = (&s.reserve_transfer, reserve_title) {
            let mut args = FluentArgs::new();
            args.set("reserve", reserve.to_string());
            args.set("amount", self.money(amount.abs()));
            args.set("end", end);
            lines.push(self.text(if amount.is_negative() { "budget-swept" } else { "budget-drawn" }, Some(&args)));
        }
        lines
    }

    /// One reserve, in a sentence or two.
    pub fn reserve_lines(&self, title: &str, s: &ReserveStatus, today: Date) -> Vec<String> {
        let mut args = FluentArgs::new();
        args.set("title", title.to_string());
        args.set("balance", self.money(s.balance_now));
        args.set("month_done", self.money(s.month_done));
        args.set("month_planned", self.money(s.month_planned));
        args.set("year_end_date", self.day_month(today.last_of_year()));
        args.set("year_end", self.money(s.year_end));
        let mut lines = vec![self.text("reserve-line", Some(&args))];
        match s.months_left {
            Some(months) => {
                // A number, for its plural: "about a month", "about 16 months".
                let mut args = FluentArgs::new();
                args.set("months", months);
                lines.push(self.text("reserve-lasts-months", Some(&args)));
            }
            None => lines.push(self.text("reserve-quiet", None)),
        }
        lines
    }
}

/// "1234567" → "1,234,567" with this language's separator.
fn group_thousands(n: i64, separator: &str) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push_str(separator);
        }
        out.push(c);
    }
    out
}

fn proof_id(proof: Proof) -> &'static str {
    match proof {
        Proof::DmarcPass => "reason-dmarc-pass",
        Proof::DmarcFail => "reason-dmarc-fail",
        Proof::DmarcFailNoPolicy => "reason-dmarc-fail-no-policy",
        Proof::DmarcFailList => "reason-dmarc-fail-list",
        Proof::DkimPass => "reason-dkim-pass",
        Proof::BothFailed => "reason-both-failed",
        Proof::NothingProves => "reason-nothing-proves",
        Proof::NoResults => "reason-no-results",
    }
}

fn kind_id(kind: CodeKind) -> &'static str {
    match kind {
        CodeKind::Code => "code-kind-code",
        CodeKind::Password => "code-kind-password",
        CodeKind::PasswordReset => "code-kind-reset",
        CodeKind::SignInLink => "code-kind-link",
        CodeKind::Confirmation => "code-kind-confirm",
    }
}

fn case_title(id: &str, cases: Option<&CaseStore>) -> String {
    cases.and_then(|c| c.get(id)).map_or_else(|| id.to_string(), |c| c.title.clone())
}

/// "fr_FR.UTF-8" → "fr"; "en-GB" → "en".
fn base_language(code: &str) -> String {
    code.split(['_', '-', '.', '@']).next().unwrap_or("").trim().to_ascii_lowercase()
}

/// The language of the session: LC_ALL, LC_MESSAGES, then LANG; English if none.
pub fn system_language() -> String {
    ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .filter_map(|v| std::env::var(v).ok())
        .map(|v| base_language(&v))
        .find(|l| !l.is_empty() && l != "c" && l != "posix")
        .unwrap_or_else(|| "en".to_string())
}

/// A probability as a whole percentage, never above what it is: 0.9496 is
/// 94, so that a doubt never reads as the spam threshold it stays below.
pub fn percent(p: f32) -> String {
    let p = (f64::from(p) * 100.0 + 1e-4).floor().clamp(0.0, 100.0);
    format!("{p:.0}")
}

fn bundle(language: &str) -> Bundle {
    let id: LanguageIdentifier = language.parse().unwrap_or_default();
    let mut bundle = Bundle::new_concurrent(vec![id]);
    // Fluent wraps variables in Unicode isolation marks for mixed scripts; they
    // show as stray characters in terminals. Off until a right-to-left language arrives.
    bundle.set_use_isolating(false);
    let source = LANGUAGES.iter().find(|(l, _)| *l == language).map_or("", |(_, s)| *s);
    let resource = FluentResource::try_new(source.to_string()).unwrap_or_else(|(partial, _)| partial);
    // A message defined twice keeps its first definition; nothing else can fail here.
    let _ = bundle.add_resource(resource);
    bundle
}

fn format(bundle: &Bundle, id: &str, attribute: Option<&str>, args: Option<&FluentArgs>) -> Option<String> {
    let message = bundle.get_message(id)?;
    let pattern = match attribute {
        Some(a) => message.get_attribute(a)?.value(),
        None => message.value()?,
    };
    let mut errors = Vec::new();
    Some(bundle.format_pattern(pattern, args, &mut errors).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_language_parses_and_has_every_message() {
        for (language, source) in LANGUAGES {
            let parsed = FluentResource::try_new(source.to_string());
            assert!(parsed.is_ok(), "{language}: syntax errors");
        }
        let english = FluentResource::try_new(LANGUAGES[0].1.to_string()).unwrap();
        let ids: Vec<String> = english
            .entries()
            .filter_map(|e| match e {
                fluent_syntax::ast::Entry::Message(m) => Some(m.id.name.to_string()),
                _ => None,
            })
            .collect();
        for (language, _) in LANGUAGES {
            let t = Translator::new(language);
            for id in &ids {
                assert!(t.bundle.has_message(id), "{language} lacks {id}");
            }
        }
    }

    /// A message said twice: the first one is shown, whichever was meant.
    #[test]
    fn no_message_twice() {
        for (language, source) in LANGUAGES {
            let parsed = FluentResource::try_new(source.to_string()).unwrap();
            let mut seen = std::collections::HashSet::new();
            for entry in parsed.entries() {
                if let fluent_syntax::ast::Entry::Message(m) = entry {
                    assert!(seen.insert(m.id.name.to_string()), "{language}: {} twice", m.id.name);
                }
            }
        }
    }

    #[test]
    fn counts_agree_with_their_noun() {
        let fr = Translator::new("fr_FR.UTF-8");
        assert_eq!(fr.language(), "fr");
        assert_eq!(fr.count(1, true, true), "Une");
        assert_eq!(fr.count(1, true, false), "Un");
        assert_eq!(fr.count(2, true, true), "Deux");
        assert_eq!(fr.count(20, false, false), "20");
        assert_eq!(Translator::new("de").language(), "en");
    }

    #[test]
    fn money_in_each_language() {
        assert_eq!(Translator::new("en").money(Money(-123456)), "−€1,234.56");
        assert_eq!(Translator::new("fr").money(Money(-123456)), "−1\u{202f}234,56\u{a0}€");
        assert_eq!(Translator::new("fr").money(Money(25000)), "250\u{a0}€");
    }

    #[test]
    fn months_short_and_counted() {
        let june: Date = "2026-06-01".parse().unwrap();
        let july: Date = "2026-07-01".parse().unwrap();
        let fr = Translator::new("fr");
        assert_ne!(fr.month_short(june), fr.month_short(july), "juin and juil. told apart");
        let status = |months: i64| ReserveStatus { id: String::new(), balance_now: Money::ZERO, month_done: Money::ZERO, month_planned: Money::ZERO, year_done: Money::ZERO, year_planned: Money::ZERO, year_end: Money::ZERO, months_left: Some(months) };
        let en = Translator::new("en");
        assert_eq!(en.reserve_lines("Savings", &status(1), june)[1], "At this pace it lasts about a month.");
        assert_eq!(en.reserve_lines("Savings", &status(16), june)[1], "At this pace it lasts about 16 months.");
        assert_eq!(fr.reserve_lines("Livret", &status(0), june)[1], "À ce rythme, elle est déjà à son plancher.");
        assert_eq!(fr.reserve_lines("Livret", &status(1), june)[1], "À ce rythme, elle tient environ un mois.");
    }

    #[test]
    fn dates_in_each_language() {
        let at: Zoned = "2026-10-06T10:00[Europe/Paris]".parse().unwrap();
        assert_eq!(Translator::new("en").when(&at), "on Tuesday 6 October at 10:00");
        assert_eq!(Translator::new("fr").when(&at), "le mardi 6 octobre à 10:00");
        assert_eq!(Translator::new("fr").date(&at, true), "mar. 6 oct. 10:00");
        // French says the first day of a month "1er".
        let first = jiff::civil::Date::constant(2026, 11, 1);
        assert_eq!(Translator::new("fr").day_month(first), "1er novembre");
        assert_eq!(Translator::new("en").day_month(first), "1 November");
    }
}

// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Invoices for work done for someone. One is made from a project's billable
//! time not billed yet: a line per task (or per note, for time noted without
//! a task), its hours at the project's rate. Numbers follow each other
//! within a year ("2026-001", "2026-002"), as French law asks, and are never
//! reused. Each invoice is kept as a record
//! (`$XDG_DATA_HOME/sioul/invoices/<number>.toml`); the time it bills carries
//! its number, so it is never billed twice; the window prints it to PDF.

use crate::config::{InvoiceSettings, data_dir};
use crate::i18n::Translator;
use crate::money::Money;
use crate::timereport::{Entry, amount, duration};
use jiff::Span;
use jiff::civil::Date;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// One line: what was done, how long, at what rate.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct InvoiceLine {
    pub label: String,
    pub minutes: u32,
    pub rate: f64,
    pub cents: i64,
}

/// Who sends it, as it was when the invoice was made.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Issuer {
    pub name: String,
    pub address: String,
    pub siret: String,
    pub vat: String,
    pub payment: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Invoice {
    pub number: String,
    /// "2026-10-05".
    pub date: String,
    /// When payment is due: thirty days after.
    pub due: String,
    pub project: String,
    pub project_title: String,
    pub client: String,
    #[serde(default)]
    pub client_address: String,
    pub lines: Vec<InvoiceLine>,
    pub total_cents: i64,
    pub currency: String,
    pub issuer: Issuer,
    /// The time it bills, by session key.
    pub sessions: Vec<String>,
    #[serde(default)]
    pub paid: bool,
}

impl Invoice {
    pub fn total(&self) -> Money {
        Money(self.total_cents)
    }
}

/// Where invoices are kept.
pub fn folder() -> PathBuf {
    data_dir().join("invoices")
}

/// Every invoice kept in `dir`, by number. A record that no longer reads
/// (edited by hand, written by a newer Sioul) still holds its number, so the
/// number is never given again, nor its file written over: it comes back
/// with its number only.
pub fn all_in(dir: &Path) -> Vec<Invoice> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    let read = |path: &Path| -> Option<Invoice> {
        let text = std::fs::read_to_string(path).ok();
        if let Some(invoice) = text.as_deref().and_then(|t| toml::from_str::<Invoice>(t).ok()) {
            return Some(invoice);
        }
        let number = text.and_then(|t| t.parse::<toml::Table>().ok()).and_then(|t| t.get("number")?.as_str().map(str::to_string));
        let number = number.or_else(|| path.file_stem().map(|s| s.to_string_lossy().into_owned()))?;
        Some(Invoice { number, ..Invoice::default() })
    };
    let mut out: Vec<Invoice> = entries.filter_map(Result::ok).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "toml")).filter_map(|p| read(&p)).collect();
    out.sort_by(|a, b| a.number.cmp(&b.number));
    out
}

/// The number after the last one of the year: `prefix` (else "2026-") and three digits.
pub fn next_number(existing: &[Invoice], prefix: &str, year: i16) -> String {
    let prefix = if prefix.trim().is_empty() { format!("{year}-") } else { prefix.trim().to_string() };
    let last = existing.iter().filter_map(|i| i.number.strip_prefix(&prefix)).filter_map(|rest| rest.parse::<u32>().ok()).max().unwrap_or(0);
    format!("{prefix}{:03}", last.saturating_add(1))
}

/// An invoice for a project's billable time not billed yet, among `entries`;
/// None when there is none.
#[allow(clippy::too_many_arguments)]
pub fn make(entries: &[Entry], project: &crate::cases::Case, client: &str, client_address: &str, settings: &InvoiceSettings, number: &str, date: Date) -> Option<Invoice> {
    let rate = project.rate.unwrap_or(settings.rate);
    let billed: Vec<&Entry> = entries.iter().filter(|e| e.project == project.id && e.billable && e.invoice.is_empty() && e.minutes > 0).collect();
    if billed.is_empty() {
        return None;
    }
    // A line per task or note, in the order they were first worked on.
    let mut lines: Vec<(i64, InvoiceLine)> = Vec::new();
    for entry in &billed {
        let label = if entry.title.trim().is_empty() { project.title.clone() } else { entry.title.clone() };
        match lines.iter_mut().find(|(_, l)| l.label == label) {
            Some((first, line)) => {
                *first = (*first).min(entry.start);
                line.minutes += entry.minutes;
            }
            None => lines.push((entry.start, InvoiceLine { label, minutes: entry.minutes, rate, cents: 0 })),
        }
    }
    lines.sort_by_key(|(first, _)| *first);
    let lines: Vec<InvoiceLine> = lines
        .into_iter()
        .map(|(_, mut line)| {
            line.cents = amount(line.minutes, line.rate).cents();
            line
        })
        .collect();
    Some(Invoice {
        number: number.to_string(),
        date: date.to_string(),
        due: date.checked_add(Span::new().days(30)).unwrap_or(date).to_string(),
        project: project.id.clone(),
        project_title: project.title.clone(),
        client: client.to_string(),
        client_address: client_address.to_string(),
        total_cents: lines.iter().map(|l| l.cents).sum(),
        lines,
        currency: if settings.currency.trim().is_empty() { "EUR".into() } else { settings.currency.trim().to_string() },
        issuer: Issuer { name: settings.name.clone(), address: settings.address.clone(), siret: settings.siret.clone(), vat: settings.vat.clone(), payment: settings.payment.clone() },
        sessions: billed.iter().map(|e| e.key.clone()).collect(),
        paid: false,
    })
}

/// Keeps an invoice in `dir`.
pub fn save_in(dir: &Path, invoice: &Invoice) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    // A number becomes a file name on every system: what Windows refuses in one
    // (":" would even write into a hidden stream) becomes "-".
    let name: String = invoice.number.chars().map(|c| if c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') { '-' } else { c }).collect();
    let path = dir.join(format!("{}.toml", name.trim_end_matches(['.', ' '])));
    let temporary = path.with_extension("toml.new");
    std::fs::write(&temporary, toml::to_string(invoice).map_err(|e| e.to_string())?).map_err(|e| format!("{}: {e}", temporary.display()))?;
    std::fs::rename(&temporary, &path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(path)
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn lines_html(text: &str) -> String {
    text.lines().map(escape).collect::<Vec<_>>().join("<br>")
}

/// The invoice as a page to print: who sends it and to whom, its lines, the
/// total, the legal mentions and how to pay.
pub fn html(invoice: &Invoice, tr: &Translator) -> String {
    let money = |cents: i64| if invoice.currency == "EUR" { tr.money(Money(cents)) } else { format!("{} {}", tr.number(Money(cents)), escape(&invoice.currency)) };
    let date = |text: &str| text.parse::<Date>().map(|d| tr.day_month(d) + " " + &d.year().to_string()).unwrap_or_else(|_| text.to_string());
    let say = |id: &str, pairs: &[(&str, String)]| {
        let mut args = crate::i18n::args();
        for (k, v) in pairs {
            args.set(k.to_string(), v.clone());
        }
        tr.text(id, Some(&args))
    };
    let rows: String = invoice
        .lines
        .iter()
        .map(|l| {
            format!(
                "<tr><td width=\"52%\">{}</td><td width=\"14%\" align=\"right\">{}</td><td width=\"17%\" align=\"right\">{}</td><td width=\"17%\" align=\"right\">{}</td></tr>",
                escape(&l.label),
                duration(l.minutes),
                money((l.rate * 100.0).round() as i64),
                money(l.cents)
            )
        })
        .collect();
    let siret = if invoice.issuer.siret.trim().is_empty() { String::new() } else { format!("<br>{}", escape(&say("invoice-siret", &[("siret", invoice.issuer.siret.clone())]))) };
    format!(
        "<html><head><meta charset=\"utf-8\"><style>\
         body {{ font-family: sans-serif; font-size: 11pt; color: #222; }}\
         h1 {{ font-size: 20pt; font-weight: normal; margin: 0 0 4pt 0; }}\
         table.lines {{ border-collapse: collapse; margin-top: 18pt; }}\
         table.lines th {{ border-bottom: 1px solid #888; font-weight: normal; color: #555; }}\
         table.lines td {{ border-bottom: 1px solid #ddd; }}\
         .total {{ font-size: 12pt; }}\
         .muted {{ color: #555; }}\
         </style></head><body>\
         <table width=\"100%\"><tr>\
         <td valign=\"top\" width=\"55%\"><b>{issuer}</b><br>{issuer_address}{siret}</td>\
         <td valign=\"top\"><span class=\"muted\">{to}</span><br><b>{client}</b><br>{client_address}</td>\
         </tr></table>\
         <p>&nbsp;</p>\
         <h1>{title}</h1>\
         <p class=\"muted\">{dated}<br>{due}<br>{project}</p>\
         <table class=\"lines\" width=\"100%\" cellpadding=\"6\" cellspacing=\"0\"><tr><th width=\"52%\" align=\"left\">{h_what}</th><th width=\"14%\" align=\"right\">{h_time}</th><th width=\"17%\" align=\"right\">{h_rate}</th><th width=\"17%\" align=\"right\">{h_amount}</th></tr>{rows}\
         <tr><td colspan=\"3\" align=\"right\" class=\"total\"><b>{h_total}</b></td><td align=\"right\" class=\"total\"><b>{total}</b></td></tr></table>\
         <p>{vat}</p>\
         <p>{payment}</p>\
         <p class=\"muted\">{late}</p>\
         </body></html>",
        issuer = escape(&invoice.issuer.name),
        issuer_address = lines_html(&invoice.issuer.address),
        to = escape(&tr.text("invoice-to", None)),
        client = escape(&invoice.client),
        client_address = lines_html(&invoice.client_address),
        title = escape(&say("invoice-title", &[("number", invoice.number.clone())])),
        dated = escape(&say("invoice-dated", &[("date", date(&invoice.date))])),
        due = escape(&say("invoice-due", &[("date", date(&invoice.due))])),
        project = escape(&say("invoice-project", &[("project", invoice.project_title.clone())])),
        h_what = escape(&tr.text("invoice-what", None)),
        h_time = escape(&tr.text("invoice-time", None)),
        h_rate = escape(&tr.text("invoice-rate", None)),
        h_amount = escape(&tr.text("invoice-amount", None)),
        h_total = escape(&tr.text("invoice-total", None)),
        total = money(invoice.total_cents),
        vat = escape(&invoice.issuer.vat),
        payment = lines_html(&invoice.issuer.payment),
        late = escape(&tr.text("invoice-late", None)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cases::Case;

    fn entry(key: &str, start: i64, minutes: u32, title: &str, invoice: &str) -> Entry {
        Entry {
            key: key.into(),
            start,
            day: Date::ZERO,
            minutes,
            project: "lumen".into(),
            task: String::new(),
            title: title.into(),
            note: String::new(),
            billable: true,
            invoice: invoice.into(),
        }
    }

    #[test]
    fn an_invoice_bills_each_hour_once() {
        let project = Case { id: "lumen".into(), title: "Studio Lumen".into(), kind: Some("project".into()), rate: Some(60.0), ..Case::default() };
        let entries = vec![
            entry("a", 10, 90, "Build the site", ""),
            entry("b", 20, 25, "Call with the client", ""),
            entry("c", 30, 30, "Build the site", ""),
            entry("d", 40, 60, "Build the site", "2026-001"),
            Entry { billable: false, ..entry("e", 50, 60, "Coffee", "") },
        ];
        let settings = InvoiceSettings { name: "Jean Exemple".into(), siret: "123 456 789 00010".into(), vat: "TVA non applicable, art. 293 B du CGI".into(), rate: 40.0, ..InvoiceSettings::default() };
        let existing = vec![Invoice { number: "2026-001".into(), ..Invoice::default() }, Invoice { number: "2025-007".into(), ..Invoice::default() }];
        let number = next_number(&existing, "", 2026);
        assert_eq!(number, "2026-002");
        let invoice = make(&entries, &project, "Studio Lumen", "1 rue de la Paix\n75002 Paris", &settings, &number, "2026-10-05".parse().unwrap()).unwrap();
        assert_eq!(invoice.lines.iter().map(|l| (l.label.as_str(), l.minutes, l.cents)).collect::<Vec<_>>(), vec![("Build the site", 120, 12000), ("Call with the client", 25, 2500)]);
        assert_eq!((invoice.total_cents, invoice.due.as_str()), (14500, "2026-11-04"));
        assert_eq!(invoice.sessions, vec!["a", "b", "c"], "billed and given time left out");
        let tr = Translator::new("fr");
        let page = html(&invoice, &tr);
        assert!(page.contains("2026-002") && page.contains("Build the site") && page.contains("293 B") && page.contains("75002 Paris"), "{page}");
        let dir = std::env::temp_dir().join(format!("sioul-invoices-{}", std::process::id()));
        save_in(&dir, &invoice).unwrap();
        assert_eq!(all_in(&dir), vec![invoice]);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(make(&entries[3..], &project, "", "", &settings, "x", "2026-10-05".parse().unwrap()).is_none());
    }

    #[test]
    fn a_number_is_never_given_twice() {
        let dir = std::env::temp_dir().join(format!("sioul-invoices-kept-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // A record broken by hand still holds its number, and its file is not written over.
        std::fs::write(dir.join("2026-003.toml"), "number = \"2026-003\"\ndate = oops\n").unwrap();
        std::fs::write(dir.join("2026-004.toml"), "not toml at all [").unwrap();
        let kept = all_in(&dir);
        assert_eq!(kept.iter().map(|i| i.number.as_str()).collect::<Vec<_>>(), ["2026-003", "2026-004"]);
        assert_eq!(next_number(&kept, "", 2026), "2026-005");
        // A number with what Windows refuses in a file name is kept under a name it takes.
        let path = save_in(&dir, &Invoice { number: "F:2026/001".into(), ..Invoice::default() }).unwrap();
        assert_eq!(path.file_name().unwrap(), "F-2026-001.toml");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

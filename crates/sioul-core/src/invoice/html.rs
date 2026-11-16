// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! An invoice as a page to print (Qt's HTML, until the issued file of
//! phase 2): who sends it and to whom with their identifiers, the delivery
//! address, the nature of the operation, the date or period of the
//! service, its lines (quantity, unit, unit price), the VAT per rate when
//! liable, the totals, the legal lines and how to pay; in the invoice's own
//! language and currency. A format-1 record prints as it always did.

use super::presets;
use super::record::{Invoice, Kind, Line, Nature};
use crate::i18n::Translator;
use crate::money::Money;
use crate::timereport::duration;
use jiff::civil::Date;

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn lines_html(text: &str) -> String {
    text.lines().map(escape).collect::<Vec<_>>().join("<br>")
}

/// The invoice's page in its own language; `fallback` (the window's) for
/// one that says none (format 1).
pub fn page(invoice: &Invoice, fallback: &Translator) -> String {
    if invoice.language.is_empty() { html(invoice, fallback) } else { html(invoice, &Translator::new(&invoice.language)) }
}

/// The invoice as a page to print, in `tr`'s language.
pub fn html(invoice: &Invoice, tr: &Translator) -> String {
    if invoice.older.is_some() { older(invoice, tr) } else { current(invoice, tr) }
}

/// A message with its arguments.
fn say(tr: &Translator, id: &str, pairs: &[(&str, String)]) -> String {
    let mut args = crate::i18n::args();
    for (k, v) in pairs {
        args.set(k.to_string(), v.clone());
    }
    tr.text(id, Some(&args))
}

fn date_of(tr: &Translator, text: &str) -> String {
    text.get(..10).and_then(|t| t.parse::<Date>().ok()).map(|d| tr.day_month(d) + " " + &d.year().to_string()).unwrap_or_else(|| text.to_string())
}

const STYLE: &str = "body { font-family: sans-serif; font-size: 10pt; color: #222; }\
     h1 { font-size: 18pt; font-weight: normal; margin: 0 0 4pt 0; }\
     table.lines { border-collapse: collapse; margin-top: 14pt; }\
     table.lines th { border-bottom: 1px solid #888; font-weight: normal; color: #555; }\
     table.lines td { border-bottom: 1px solid #ddd; }\
     .total { font-size: 11pt; }\
     .muted { color: #555; }\
     .small { font-size: 8.5pt; color: #444; }";

/// A format-2 record's page.
fn current(invoice: &Invoice, tr: &Translator) -> String {
    let language = tr.language().to_string();
    let money = |cents: i64| escape(&tr.money_in(Money(cents), &invoice.currency));
    let preset = presets::get(&invoice.issuer.preset);
    let home = invoice.issuer.address.country.clone();
    // An identifier as printed: its preset's words before it ("SIREN", "RCS"), else its kind.
    let id_line = |kind: &str, value: &str| -> String {
        let before = preset.id_kind(kind).map(|k| if k.before.is_empty() { k.name.get(&language).to_string() } else { k.before.get(&language).to_string() }).unwrap_or_else(|| if kind == "other" { String::new() } else { kind.to_uppercase() });
        if before.is_empty() { escape(value) } else { format!("{} {}", escape(&before), escape(value)) }
    };
    let issuer = &invoice.issuer;
    let mut seller: Vec<String> = vec![format!("<b>{}</b>", escape(&issuer.name))];
    if !issuer.trade_name.is_empty() {
        seller.push(escape(&issuer.trade_name));
    }
    // The issuer's country said only to a client abroad, the client's only when abroad.
    let client_country = if invoice.client.address.country.trim().is_empty() { home.clone() } else { invoice.client.address.country.clone() };
    seller.push(lines_html(&issuer.address.text(&client_country)));
    seller.extend(issuer.ids.iter().filter(|i| i.printed && !i.value.trim().is_empty()).map(|i| id_line(&i.kind, &i.value)));
    if !issuer.email.is_empty() {
        seller.push(escape(&issuer.email));
    }
    let client = &invoice.client;
    let mut buyer: Vec<String> = vec![format!("<span class=\"muted\">{}</span>", escape(&tr.text(if invoice.kind == Kind::CreditNote { "invoice-credit-to" } else { "invoice-to" }, None))), format!("<b>{}</b>", lines_html(&client.name))];
    buyer.push(lines_html(&client.address.text(&home)));
    buyer.extend(client.ids.iter().filter(|i| i.printed && !i.value.trim().is_empty()).map(|i| id_line(&i.kind, &i.value)));
    if let Some(delivery) = client.delivery.as_ref().filter(|d| !d.is_empty() && **d != client.address) {
        buyer.push(format!("<span class=\"muted\">{}</span><br>{}", escape(&tr.text("invoice-delivery", None)), lines_html(&delivery.text(&home))));
    }
    let title = if invoice.number.is_empty() {
        tr.text(&format!("invoice-draft-title-{}", invoice.kind.id()), None)
    } else {
        say(tr, &format!("invoice-title-{}", invoice.kind.id()), &[("number", invoice.number.clone())])
    };
    let mut facts: Vec<String> = Vec::new();
    if !invoice.date.is_empty() {
        facts.push(say(tr, "invoice-dated", &[("date", date_of(tr, &invoice.date))]));
    }
    match (invoice.period_from.as_str(), invoice.period_to.as_str()) {
        ("", _) => {}
        (from, to) if to.is_empty() || from == to => facts.push(say(tr, "invoice-period-day", &[("date", date_of(tr, from))])),
        (from, to) => facts.push(say(tr, "invoice-period", &[("from", date_of(tr, from)), ("to", date_of(tr, to))])),
    }
    if let Some(cited) = &invoice.corrects {
        facts.push(say(tr, "invoice-corrects", &[("number", cited.number.clone()), ("date", date_of(tr, &cited.date))]));
    }
    if invoice.kind != Kind::CreditNote && !invoice.due.is_empty() {
        facts.push(say(tr, "invoice-due", &[("date", date_of(tr, &invoice.due))]));
    }
    // The nature of the operation: goods, services, or both (France, from September 2027).
    let goods = invoice.lines.iter().any(|l| l.nature == Nature::Goods);
    let services = invoice.lines.iter().any(|l| l.nature == Nature::Service);
    if goods || services {
        facts.push(tr.text(
            match (goods, services) {
                (true, true) => "invoice-nature-both",
                (true, false) => "invoice-nature-goods",
                _ => "invoice-nature-services",
            },
            None,
        ));
    }
    if !invoice.project_title.is_empty() {
        facts.push(say(tr, "invoice-project", &[("project", invoice.project_title.clone())]));
    }
    if !invoice.order.is_empty() {
        facts.push(say(tr, "invoice-order", &[("order", invoice.order.clone())]));
    }
    let liable = issuer.scheme == "liable";
    let quantity = |line: &Line| -> String {
        match line.minutes() {
            Some(minutes) => duration(minutes),
            None => {
                let mut args = crate::i18n::args();
                args.set("count", if line.quantity % 1000 == 0 { line.quantity / 1000 } else { 2 });
                let unit = tr.text(&format!("invoice-unit-{}", line.unit), Some(&args));
                let unit = if unit.starts_with("invoice-unit-") { String::new() } else { unit };
                [tr.thousandths(line.quantity), unit].into_iter().filter(|t| !t.is_empty()).collect::<Vec<_>>().join(" ")
            }
        }
    };
    let price = |line: &Line| -> String {
        if line.minutes().is_some() {
            // Minutes at an hourly rate: the price of an hour.
            say(tr, "invoice-per-hour", &[("price", tr.money_in(Money(line.price * 60 / line.base.max(1)), &invoice.currency))])
        } else {
            tr.money_in(Money(line.price), &invoice.currency)
        }
    };
    let rows: String = invoice
        .lines
        .iter()
        .map(|l| {
            let details = if l.details.is_empty() { String::new() } else { format!("<br><span class=\"small\">{}</span>", lines_html(&l.details)) };
            let discount = if l.discount == 0 { String::new() } else { format!("<br><span class=\"small\">{}</span>", escape(&say(tr, "invoice-line-discount", &[("amount", tr.money_in(Money(l.discount), &invoice.currency))]))) };
            let vat = if liable { format!("<td align=\"right\">{}</td>", if l.category == "S" { escape(&tr.percent_bp(l.rate)) } else { escape(&l.category) }) } else { String::new() };
            format!("<tr><td>{}{details}{discount}</td><td align=\"right\">{}</td><td align=\"right\">{}</td>{vat}<td align=\"right\">{}</td></tr>", escape(&l.description), escape(&quantity(l)), escape(&price(l)), money(l.amount))
        })
        .collect();
    let columns = if liable { 5 } else { 4 };
    let vat_head = if liable { format!("<th align=\"right\">{}</th>", escape(&tr.text("invoice-vat-rate", None))) } else { String::new() };
    let mut sums: Vec<(String, String)> = Vec::new();
    let totals = &invoice.totals;
    if liable {
        sums.push((tr.text("invoice-total-without-tax", None), money(totals.without_tax)));
        for b in totals.breakdown.iter().filter(|b| b.category == "S") {
            sums.push((say(tr, "invoice-vat-on", &[("rate", tr.percent_bp(b.rate)), ("base", tr.money_in(Money(b.base), &invoice.currency))]), money(b.tax)));
        }
        sums.push((tr.text("invoice-total-with-tax", None), money(totals.with_tax)));
    } else {
        sums.push((tr.text("invoice-total", None), money(totals.with_tax)));
    }
    if !invoice.deposits.is_empty() {
        let numbers = invoice.deposits.iter().map(|d| d.number.clone()).collect::<Vec<_>>().join(", ");
        sums.push((say(tr, "invoice-deposits-deducted", &[("numbers", numbers)]), money(-totals.deposits)));
        sums.push((tr.text("invoice-amount-due", None), money(totals.due)));
    }
    let sums_html: String = sums
        .iter()
        .enumerate()
        .map(|(i, (label, amount))| {
            let strong = i + 1 == sums.len();
            let (open, close) = if strong { ("<b>", "</b>") } else { ("", "") };
            format!("<tr><td colspan=\"{}\" align=\"right\" class=\"total\">{open}{}{close}</td><td align=\"right\" class=\"total\">{open}{amount}{close}</td></tr>", columns - 1, escape(label))
        })
        .collect();
    // Why a part carries no VAT: the franchise's mention, the reverse charge, an exemption.
    let mut mentions: Vec<String> = Vec::new();
    for b in &totals.breakdown {
        if !b.reason.trim().is_empty() && !mentions.contains(&b.reason) {
            mentions.push(b.reason.clone());
        }
    }
    let notes: Vec<String> = invoice.notes.iter().filter(|n| !n.text.trim().is_empty()).map(|n| lines_html(&n.text)).collect();
    let mut pay: Vec<String> = Vec::new();
    if invoice.kind != Kind::CreditNote {
        for bank in &issuer.banks {
            let holder = if bank.holder.is_empty() { String::new() } else { format!("{} · ", escape(&bank.holder)) };
            let bic = if bank.bic.is_empty() { String::new() } else { format!(" · BIC {}", escape(&bank.bic)) };
            pay.push(format!("{holder}IBAN {}{bic}", escape(&bank.iban)));
        }
        if !issuer.payment.is_empty() {
            pay.push(lines_html(&issuer.payment));
        }
    }
    if !issuer.insurance.is_empty() {
        pay.push(lines_html(&say(tr, "invoice-insurance", &[("insurance", issuer.insurance.clone())])));
    }
    let block = |items: &[String], class: &str| if items.is_empty() { String::new() } else { format!("<p class=\"{class}\">{}</p>", items.join("<br>")) };
    format!(
        "<html><head><meta charset=\"utf-8\"><style>{STYLE}</style></head><body>\
         <table width=\"100%\"><tr>\
         <td valign=\"top\" width=\"55%\">{seller}</td>\
         <td valign=\"top\">{buyer}</td>\
         </tr></table>\
         <p>&nbsp;</p>\
         <h1>{title}</h1>\
         <p class=\"muted\">{facts}</p>\
         <table class=\"lines\" width=\"100%\" cellpadding=\"5\" cellspacing=\"0\"><tr><th align=\"left\" width=\"46%\">{h_what}</th><th align=\"right\">{h_quantity}</th><th align=\"right\">{h_price}</th>{vat_head}<th align=\"right\">{h_amount}</th></tr>{rows}{sums_html}</table>\
         {mentions}{pay}{notes}\
         </body></html>",
        seller = seller.into_iter().filter(|l| !l.is_empty()).collect::<Vec<_>>().join("<br>"),
        buyer = buyer.into_iter().filter(|l| !l.is_empty()).collect::<Vec<_>>().join("<br>"),
        title = escape(&title),
        facts = facts.iter().map(|f| escape(f)).collect::<Vec<_>>().join("<br>"),
        h_what = escape(&tr.text("invoice-description", None)),
        h_quantity = escape(&tr.text("invoice-quantity", None)),
        h_price = escape(&tr.text("invoice-unit-price", None)),
        h_amount = escape(&tr.text("invoice-amount", None)),
        mentions = block(&mentions.iter().map(|m| escape(m)).collect::<Vec<_>>(), ""),
        pay = block(&pay, ""),
        notes = block(&notes, "small"),
    )
}

/// A format-1 record's page, as Sioul printed it before November 2026.
fn older(invoice: &Invoice, tr: &Translator) -> String {
    let older = invoice.older.clone().unwrap_or_default();
    let money = |cents: i64| if invoice.currency == "EUR" { tr.money(Money(cents)) } else { format!("{} {}", tr.number(Money(cents)), escape(&invoice.currency)) };
    let rows: String = invoice
        .lines
        .iter()
        .map(|l| {
            format!(
                "<tr><td width=\"52%\">{}</td><td width=\"14%\" align=\"right\">{}</td><td width=\"17%\" align=\"right\">{}</td><td width=\"17%\" align=\"right\">{}</td></tr>",
                escape(&l.description),
                duration(l.minutes().unwrap_or(0)),
                money(l.price),
                money(l.amount)
            )
        })
        .collect();
    let siret = if older.siret.trim().is_empty() { String::new() } else { format!("<br>{}", escape(&say(tr, "invoice-siret", &[("siret", older.siret.clone())]))) };
    let period = match (invoice.period_from.as_str(), invoice.period_to.as_str()) {
        ("", _) => String::new(),
        (from, to) if to.is_empty() || from == to => format!("<br>{}", escape(&say(tr, "invoice-period-day", &[("date", date_of(tr, from))]))),
        (from, to) => format!("<br>{}", escape(&say(tr, "invoice-period", &[("from", date_of(tr, from)), ("to", date_of(tr, to))]))),
    };
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
         <p class=\"muted\">{dated}{period}<br>{due}<br>{project}</p>\
         <table class=\"lines\" width=\"100%\" cellpadding=\"6\" cellspacing=\"0\"><tr><th width=\"52%\" align=\"left\">{h_what}</th><th width=\"14%\" align=\"right\">{h_time}</th><th width=\"17%\" align=\"right\">{h_rate}</th><th width=\"17%\" align=\"right\">{h_amount}</th></tr>{rows}\
         <tr><td colspan=\"3\" align=\"right\" class=\"total\"><b>{h_total}</b></td><td align=\"right\" class=\"total\"><b>{total}</b></td></tr></table>\
         <p>{vat}</p>\
         <p>{payment}</p>\
         <p class=\"muted\">{late}</p>\
         </body></html>",
        issuer = escape(&invoice.issuer.name),
        issuer_address = lines_html(&invoice.issuer.address.text("")),
        to = escape(&tr.text("invoice-to", None)),
        client = escape(&invoice.client.name),
        client_address = lines_html(&invoice.client.address.text("")),
        title = escape(&say(tr, "invoice-title", &[("number", invoice.number.clone())])),
        dated = escape(&say(tr, "invoice-dated", &[("date", date_of(tr, &invoice.date))])),
        due = escape(&say(tr, "invoice-due", &[("date", date_of(tr, &invoice.due))])),
        project = escape(&say(tr, "invoice-project", &[("project", invoice.project_title.clone())])),
        h_what = escape(&tr.text("invoice-what", None)),
        h_time = escape(&tr.text("invoice-time", None)),
        h_rate = escape(&tr.text("invoice-rate", None)),
        h_amount = escape(&tr.text("invoice-amount", None)),
        h_total = escape(&tr.text("invoice-total", None)),
        total = money(invoice.totals.due),
        vat = escape(&older.vat),
        payment = lines_html(&older.payment),
        late = escape(&tr.text("invoice-late", None)),
    )
}

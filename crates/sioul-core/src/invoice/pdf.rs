// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The issued invoice as a file (docs/projects.md, "The issued file"): a
//! PDF/A-3b on A4, written by krilla in Rust, its text in Liberation Sans
//! (regular and bold, embedded and subset; SIL Open Font License 1.1, its
//! licence in `data/fonts/LICENSE-Liberation`), its colours in sRGB with
//! their output intent, its Factur-X XML attached as `factur-x.xml` with the
//! relationship *Alternative* (the PDF and the XML say the same invoice), and
//! the Factur-X properties in its XMP (`pdfa`). A euro invoice with an IBAN
//! carries the EPC payment QR code, drawn as squares (`epc`).
//!
//! The page says what `html::current` says for a format-2 record, word for
//! word, in the invoice's own language and currency: who sends it and to
//! whom with their identifiers, the delivery address, the dates, the nature
//! of the operation, the lines, the VAT per rate when liable, the deposits
//! deducted, the legal mentions, how to pay, the notes; a test holds the two
//! to the same words. Written once, at issue (`kept`), and never again: a
//! reprint gives back that file. Format-1 records keep their own print path.
//!
//! Liberation Sans 2.1.5, byte for byte as released
//! (github.com/liberationfonts/liberation-fonts, liberation-fonts-ttf-2.1.5):
//! Regular SHA-256 76d04c18…de8ee8, Bold 788abee4…a9173. A character it
//! cannot print (a script it lacks) is said before issue (`unprintable`),
//! since PDF/A forbids drawing a missing glyph; so is right-to-left writing
//! (`right_to_left`), which would print backwards without Unicode's
//! bidirectional ordering.

use super::cii;
use super::epc::{self, Matrix};
use super::presets;
use super::record::{Invoice, Kind, Line, Nature};
use crate::i18n::Translator;
use crate::money::Money;
use crate::timereport::duration;
use jiff::civil::Date;
use krilla::color::rgb;
use krilla::configure::{Archival, ConfigurationBuilder};
use krilla::embed::{AssociationKind, EmbeddedFile, MimeType};
use krilla::geom::{PathBuilder, Point, Rect, Size};
use krilla::metadata::{DateTime, Metadata};
use krilla::num::NormalizedF32;
use krilla::page::PageSettings;
use krilla::paint::{Fill, FillRule, Stroke};
use krilla::text::{Font, GlyphId, KrillaGlyph};
use krilla::{Data, Document, SerializeSettings};

const REGULAR: &[u8] = include_bytes!("../../data/fonts/LiberationSans-Regular.ttf");
const BOLD: &[u8] = include_bytes!("../../data/fonts/LiberationSans-Bold.ttf");

/// A4, in points.
const WIDTH: f32 = 595.28;
const HEIGHT: f32 = 841.89;
/// 18 mm on each side, as the window's printing had it.
const MARGIN: f32 = 51.0;
const CONTENT: f32 = WIDTH - 2.0 * MARGIN;
/// The lowest a line of text may reach; the page's number sits below.
const BOTTOM: f32 = HEIGHT - MARGIN;
/// The QR code's square, its quiet zone included: about 25 mm, above the 2 cm the Belgian banks ask.
const QR_SIDE: f32 = 72.0;
const CELL_PADDING: f32 = 4.0;

/// What issuing asks of the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Options {
    /// The EPC payment QR code on euro invoices with an IBAN (`[invoice] qr`, on when unsaid).
    pub qr: bool,
}

impl Default for Options {
    fn default() -> Options {
        Options { qr: true }
    }
}

/// The file made: the PDF, the XML it carries, or why it carries none.
#[derive(Debug, Clone)]
pub struct Rendered {
    pub pdf: Vec<u8>,
    pub xml: Option<String>,
    pub without: Option<cii::Without>,
}

/// A text's weight and colour, as the page's style sheet had them.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Style {
    size: f32,
    bold: bool,
    ink: (u8, u8, u8),
}

const TEXT: (u8, u8, u8) = (0x22, 0x22, 0x22);
const MUTED: (u8, u8, u8) = (0x55, 0x55, 0x55);
const SMALL_INK: (u8, u8, u8) = (0x44, 0x44, 0x44);
const RULE: (u8, u8, u8) = (0x88, 0x88, 0x88);
const FAINT: (u8, u8, u8) = (0xdd, 0xdd, 0xdd);
const BLACK: (u8, u8, u8) = (0, 0, 0);

const BODY: Style = Style { size: 10.0, bold: false, ink: TEXT };
const STRONG: Style = Style { size: 10.0, bold: true, ink: TEXT };
const QUIET: Style = Style { size: 10.0, bold: false, ink: MUTED };
const SMALL: Style = Style { size: 8.5, bold: false, ink: SMALL_INK };
const TITLE: Style = Style { size: 18.0, bold: false, ink: TEXT };
const SUM: Style = Style { size: 11.0, bold: false, ink: TEXT };
const SUM_STRONG: Style = Style { size: 11.0, bold: true, ink: TEXT };
const FOOT: Style = Style { size: 8.5, bold: false, ink: MUTED };

impl Style {
    fn leading(self) -> f32 {
        self.size * 1.25
    }
}

/// What the page says, in order: the words of `html::current`.
#[derive(Debug, Default)]
struct Content {
    seller: Vec<(String, Style)>,
    buyer: Vec<(String, Style)>,
    title: String,
    facts: Vec<String>,
    heads: Vec<String>,
    rows: Vec<Row>,
    sums: Vec<(String, String)>,
    mentions: Vec<String>,
    pay: Vec<String>,
    notes: Vec<String>,
}

#[derive(Debug, Default)]
struct Row {
    description: String,
    small: Vec<String>,
    cells: Vec<String>,
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

/// The page's words for a format-2 record, as `html::current` says them.
fn content(invoice: &Invoice, tr: &Translator) -> Content {
    let language = tr.language().to_string();
    let money = |cents: i64| tr.money_in(Money(cents), &invoice.currency);
    let preset = presets::get(&invoice.issuer.preset);
    let home = invoice.issuer.address.country.clone();
    let id_line = |kind: &str, value: &str| -> String {
        let before = preset.id_kind(kind).map(|k| if k.before.is_empty() { k.name.get(&language).to_string() } else { k.before.get(&language).to_string() }).unwrap_or_else(|| if kind == "other" { String::new() } else { kind.to_uppercase() });
        if before.is_empty() { value.to_string() } else { format!("{before} {value}") }
    };
    let mut c = Content::default();
    let issuer = &invoice.issuer;
    c.seller.push((issuer.name.clone(), STRONG));
    if !issuer.trade_name.is_empty() {
        c.seller.push((issuer.trade_name.clone(), BODY));
    }
    let client_country = if invoice.client.address.country.trim().is_empty() { home.clone() } else { invoice.client.address.country.clone() };
    c.seller.push((issuer.address.text(&client_country), BODY));
    c.seller.extend(issuer.ids.iter().filter(|i| i.printed && !i.value.trim().is_empty()).map(|i| (id_line(&i.kind, &i.value), BODY)));
    if !issuer.email.is_empty() {
        c.seller.push((issuer.email.clone(), BODY));
    }
    let client = &invoice.client;
    c.buyer.push((tr.text(if invoice.kind == Kind::CreditNote { "invoice-credit-to" } else { "invoice-to" }, None), QUIET));
    c.buyer.push((client.name.clone(), STRONG));
    c.buyer.push((client.address.text(&home), BODY));
    c.buyer.extend(client.ids.iter().filter(|i| i.printed && !i.value.trim().is_empty()).map(|i| (id_line(&i.kind, &i.value), BODY)));
    if let Some(delivery) = client.delivery.as_ref().filter(|d| !d.is_empty() && **d != client.address) {
        c.buyer.push((tr.text("invoice-delivery", None), QUIET));
        c.buyer.push((delivery.text(&home), BODY));
    }
    c.seller.retain(|(t, _)| !t.trim().is_empty());
    c.buyer.retain(|(t, _)| !t.trim().is_empty());
    c.title = if invoice.number.is_empty() {
        tr.text(&format!("invoice-draft-title-{}", invoice.kind.id()), None)
    } else {
        say(tr, &format!("invoice-title-{}", invoice.kind.id()), &[("number", invoice.number.clone())])
    };
    if !invoice.date.is_empty() {
        c.facts.push(say(tr, "invoice-dated", &[("date", date_of(tr, &invoice.date))]));
    }
    match (invoice.period_from.as_str(), invoice.period_to.as_str()) {
        ("", _) => {}
        (from, to) if to.is_empty() || from == to => c.facts.push(say(tr, "invoice-period-day", &[("date", date_of(tr, from))])),
        (from, to) => c.facts.push(say(tr, "invoice-period", &[("from", date_of(tr, from)), ("to", date_of(tr, to))])),
    }
    if let Some(cited) = &invoice.corrects {
        c.facts.push(say(tr, "invoice-corrects", &[("number", cited.number.clone()), ("date", date_of(tr, &cited.date))]));
    }
    if invoice.kind != Kind::CreditNote && !invoice.due.is_empty() {
        c.facts.push(say(tr, "invoice-due", &[("date", date_of(tr, &invoice.due))]));
    }
    let goods = invoice.lines.iter().any(|l| l.nature == Nature::Goods);
    let services = invoice.lines.iter().any(|l| l.nature == Nature::Service);
    if goods || services {
        c.facts.push(tr.text(
            match (goods, services) {
                (true, true) => "invoice-nature-both",
                (true, false) => "invoice-nature-goods",
                _ => "invoice-nature-services",
            },
            None,
        ));
    }
    if !invoice.project_title.is_empty() {
        c.facts.push(say(tr, "invoice-project", &[("project", invoice.project_title.clone())]));
    }
    if !invoice.order.is_empty() {
        c.facts.push(say(tr, "invoice-order", &[("order", invoice.order.clone())]));
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
            say(tr, "invoice-per-hour", &[("price", tr.money_in(Money(line.price * 60 / line.base.max(1)), &invoice.currency))])
        } else {
            tr.money_in(Money(line.price), &invoice.currency)
        }
    };
    c.heads = vec![tr.text("invoice-description", None), tr.text("invoice-quantity", None), tr.text("invoice-unit-price", None)];
    if liable {
        c.heads.push(tr.text("invoice-vat-rate", None));
    }
    c.heads.push(tr.text("invoice-amount", None));
    for l in &invoice.lines {
        let mut small: Vec<String> = l.details.lines().map(str::to_string).collect();
        if l.discount != 0 {
            small.push(say(tr, "invoice-line-discount", &[("amount", tr.money_in(Money(l.discount), &invoice.currency))]));
        }
        let mut cells = vec![quantity(l), price(l)];
        if liable {
            cells.push(if l.category == "S" { tr.percent_bp(l.rate) } else { l.category.clone() });
        }
        cells.push(money(l.amount));
        c.rows.push(Row { description: l.description.clone(), small, cells });
    }
    let totals = &invoice.totals;
    if liable {
        c.sums.push((tr.text("invoice-total-without-tax", None), money(totals.without_tax)));
        for b in totals.breakdown.iter().filter(|b| b.category == "S") {
            c.sums.push((say(tr, "invoice-vat-on", &[("rate", tr.percent_bp(b.rate)), ("base", tr.money_in(Money(b.base), &invoice.currency))]), money(b.tax)));
        }
        c.sums.push((tr.text("invoice-total-with-tax", None), money(totals.with_tax)));
    } else {
        c.sums.push((tr.text("invoice-total", None), money(totals.with_tax)));
    }
    if !invoice.deposits.is_empty() {
        let numbers = invoice.deposits.iter().map(|d| d.number.clone()).collect::<Vec<_>>().join(", ");
        c.sums.push((say(tr, "invoice-deposits-deducted", &[("numbers", numbers)]), money(-totals.deposits)));
        c.sums.push((tr.text("invoice-amount-due", None), money(totals.due)));
    }
    for b in &totals.breakdown {
        if !b.reason.trim().is_empty() && !c.mentions.contains(&b.reason) {
            c.mentions.push(b.reason.clone());
        }
    }
    if invoice.kind != Kind::CreditNote {
        for bank in &issuer.banks {
            let holder = if bank.holder.is_empty() { String::new() } else { format!("{} · ", bank.holder) };
            let bic = if bank.bic.is_empty() { String::new() } else { format!(" · BIC {}", bank.bic) };
            c.pay.push(format!("{holder}IBAN {}{bic}", bank.iban));
        }
        if !issuer.payment.is_empty() {
            c.pay.push(issuer.payment.clone());
        }
    }
    if !issuer.insurance.is_empty() {
        c.pay.push(say(tr, "invoice-insurance", &[("insurance", issuer.insurance.clone())]));
    }
    c.notes = invoice.notes.iter().filter(|n| !n.text.trim().is_empty()).map(|n| n.text.clone()).collect();
    c
}

impl Content {
    /// Every text the page prints, as written (lines unwrapped).
    fn texts(&self) -> Vec<&str> {
        let mut all: Vec<&str> = self.seller.iter().chain(&self.buyer).map(|(t, _)| t.as_str()).collect();
        all.push(&self.title);
        all.extend(self.facts.iter().map(String::as_str));
        all.extend(self.heads.iter().map(String::as_str));
        for row in &self.rows {
            all.push(&row.description);
            all.extend(row.small.iter().map(String::as_str));
            all.extend(row.cells.iter().map(String::as_str));
        }
        for (label, amount) in &self.sums {
            all.push(label);
            all.push(amount);
        }
        all.extend(self.mentions.iter().chain(&self.pay).chain(&self.notes).map(String::as_str));
        all
    }
}

/// The two faces of Liberation Sans, for shaping and for the PDF.
struct Fonts {
    regular: Font,
    bold: Font,
    faces: [rustybuzz::Face<'static>; 2],
}

impl Fonts {
    fn load() -> Result<Fonts, String> {
        let font = |data: &'static [u8]| Font::new(Data::from(data), 0).ok_or_else(|| "Liberation Sans does not read".to_string());
        let face = |data: &'static [u8]| rustybuzz::Face::from_slice(data, 0).ok_or_else(|| "Liberation Sans does not read".to_string());
        Ok(Fonts { regular: font(REGULAR)?, bold: font(BOLD)?, faces: [face(REGULAR)?, face(BOLD)?] })
    }

    fn face(&self, bold: bool) -> &rustybuzz::Face<'static> {
        &self.faces[usize::from(bold)]
    }

    /// `text` as the font prints it: a character it lacks replaced by its
    /// nearest (a non-breaking hyphen by a hyphen, a narrow space by a
    /// space), invisible ones and controls taken out.
    fn printable(&self, text: &str, bold: bool) -> String {
        let face = self.face(bold);
        text.chars()
            .filter_map(|c| match c {
                '\t' => Some(' '),
                '\u{00AD}' | '\u{200B}' | '\u{200C}' | '\u{200D}' | '\u{2060}' | '\u{FEFF}' | '\u{2066}'..='\u{2069}' | '\u{200E}' | '\u{200F}' => None,
                c if c.is_control() => None,
                c if face.glyph_index(c).is_some() => Some(c),
                '\u{2010}' | '\u{2011}' | '\u{2012}' => Some('-'),
                '\u{202F}' | '\u{2007}' => Some('\u{00A0}'),
                '\u{2000}'..='\u{200A}' | '\u{205F}' | '\u{3000}' => Some(' '),
                '\u{2212}' => Some('-'),
                c => Some(c),
            })
            .collect()
    }

    /// The characters of `text` the font has no glyph for, once made printable.
    fn missing(&self, text: &str, bold: bool) -> Vec<char> {
        let face = self.face(bold);
        self.printable(text, bold).chars().filter(|c| face.glyph_index(*c).is_none()).collect()
    }

    /// The glyphs of a printable line, shaped as krilla would.
    fn glyphs(&self, text: &str, bold: bool) -> Vec<KrillaGlyph> {
        let face = self.face(bold);
        let units = face.units_per_em() as f32;
        let mut buffer = rustybuzz::UnicodeBuffer::new();
        buffer.push_str(text);
        buffer.guess_segment_properties();
        let forward = matches!(buffer.direction(), rustybuzz::Direction::LeftToRight | rustybuzz::Direction::TopToBottom | rustybuzz::Direction::Invalid);
        let output = rustybuzz::shape(face, &[], buffer);
        let (infos, positions) = (output.glyph_infos(), output.glyph_positions());
        (0..output.len())
            .map(|i| {
                let start = infos[i].cluster as usize;
                // A glyph's text: its cluster, up to the next cluster in reading order.
                let end = if forward { infos[i + 1..].iter().map(|g| g.cluster as usize).find(|c| *c > start) } else { infos[..i].iter().rev().map(|g| g.cluster as usize).find(|c| *c > start) }.unwrap_or(text.len());
                let p = positions[i];
                KrillaGlyph::new(GlyphId::new(infos[i].glyph_id), p.x_advance as f32 / units, p.x_offset as f32 / units, p.y_offset as f32 / units, p.y_advance as f32 / units, start..end.max(start), None)
            })
            .collect()
    }

    fn width(&self, text: &str, style: Style) -> f32 {
        let face = self.face(style.bold);
        let units = face.units_per_em() as f32;
        let mut buffer = rustybuzz::UnicodeBuffer::new();
        buffer.push_str(&self.printable(text, style.bold));
        buffer.guess_segment_properties();
        let output = rustybuzz::shape(face, &[], buffer);
        output.glyph_positions().iter().map(|p| p.x_advance as f32).sum::<f32>() / units * style.size
    }

    /// `text` on lines no wider than `width`: its own lines, each broken at
    /// spaces, a word wider than the line broken where it must.
    fn wrap(&self, text: &str, width: f32, style: Style) -> Vec<String> {
        let mut out = Vec::new();
        for paragraph in text.lines() {
            let mut line = String::new();
            for word in paragraph.split(' ') {
                let tried = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
                if self.width(&tried, style) <= width || line.is_empty() && self.width(word, style) <= width {
                    line = tried;
                    continue;
                }
                if !line.is_empty() {
                    out.push(std::mem::take(&mut line));
                }
                // A word wider than the line: as many characters as fit, then the rest.
                let mut piece = String::new();
                for c in word.chars() {
                    piece.push(c);
                    if self.width(&piece, style) > width && piece.chars().count() > 1 {
                        piece.pop();
                        out.push(std::mem::take(&mut piece));
                        piece.push(c);
                    }
                }
                line = piece;
            }
            out.push(line);
        }
        if out.is_empty() {
            out.push(String::new());
        }
        out
    }
}

/// What a page holds: text, a rule, the QR code's squares.
#[derive(Debug, Clone)]
enum Mark {
    /// `furniture`: the page's own words (its number, the QR code's caption), not the invoice's.
    Text { x: f32, baseline: f32, style: Style, text: String, furniture: bool },
    Rule { x1: f32, x2: f32, y: f32, ink: (u8, u8, u8), width: f32 },
    Qr { x: f32, y: f32, side: f32, matrix: Matrix },
}

/// The pages as they fill, top to bottom.
struct Layout<'a> {
    fonts: &'a Fonts,
    pages: Vec<Vec<Mark>>,
    /// The top of what comes next on the current page.
    y: f32,
}

impl<'a> Layout<'a> {
    fn new(fonts: &'a Fonts) -> Layout<'a> {
        Layout { fonts, pages: vec![Vec::new()], y: MARGIN }
    }

    fn page(&mut self) -> &mut Vec<Mark> {
        self.pages.last_mut().expect("a page")
    }

    fn new_page(&mut self) {
        self.pages.push(Vec::new());
        self.y = MARGIN;
    }

    /// Room for `height` more on this page, else a new page.
    fn room(&mut self, height: f32) -> bool {
        if self.y + height > BOTTOM && self.y > MARGIN {
            self.new_page();
            return true;
        }
        false
    }

    fn text(&mut self, x: f32, top: f32, style: Style, text: &str) {
        if text.is_empty() {
            return;
        }
        let baseline = top + style.size * 0.95;
        let printable = self.fonts.printable(text, style.bold);
        self.page().push(Mark::Text { x, baseline, style, text: printable, furniture: false });
    }

    /// Text ending at `right`.
    fn text_right(&mut self, right: f32, top: f32, style: Style, text: &str) {
        let width = self.fonts.width(text, style);
        self.text(right - width, top, style, text);
    }

    /// A block of text from `x`, at most `width` wide, on as many lines as it takes.
    fn block(&mut self, x: f32, width: f32, style: Style, text: &str) {
        for line in self.fonts.wrap(text, width, style) {
            self.room(style.leading());
            let top = self.y;
            self.text(x, top, style, &line);
            self.y += style.leading();
        }
    }

    fn rule(&mut self, x1: f32, x2: f32, y: f32, ink: (u8, u8, u8), width: f32) {
        self.page().push(Mark::Rule { x1, x2, y, ink, width });
    }
}

/// The table's columns: the description, then the figures, right-aligned.
fn columns(count: usize) -> Vec<(f32, f32)> {
    let first = CONTENT * 0.46;
    let rest = (CONTENT - first) / (count.max(2) - 1) as f32;
    (0..count).map(|i| if i == 0 { (MARGIN, first) } else { (MARGIN + first + rest * (i - 1) as f32, rest) }).collect()
}

/// The table's head, on the current page.
/// Each column's head on as many lines as its width takes ("Prix unitaire
/// HT" on two in a French table under VAT), never into its neighbour.
fn heads(fonts: &Fonts, c: &Content, cols: &[(f32, f32)]) -> Vec<Vec<String>> {
    c.heads.iter().zip(cols).map(|(h, (_, w))| fonts.wrap(h, w - 2.0 * CELL_PADDING, QUIET)).collect()
}

/// The head's height, its padding and rule included.
fn head_height(fonts: &Fonts, c: &Content, cols: &[(f32, f32)]) -> f32 {
    2.0 * CELL_PADDING + heads(fonts, c, cols).iter().map(Vec::len).max().unwrap_or(1) as f32 * QUIET.leading()
}

/// The table's head on the current page; `again` on a page after the first,
/// where its words are the page's own (said once, on the first).
fn head(l: &mut Layout, c: &Content, cols: &[(f32, f32)], again: bool) {
    let first = l.page().len();
    let top = l.y + CELL_PADDING;
    let mut bottom = top;
    for (i, (lines, (x, w))) in heads(l.fonts, c, cols).iter().zip(cols).enumerate() {
        let mut y = top;
        for line in lines {
            if i == 0 {
                l.text(x + CELL_PADDING, y, QUIET, line);
            } else {
                l.text_right(x + w - CELL_PADDING, y, QUIET, line);
            }
            y += QUIET.leading();
        }
        bottom = bottom.max(y);
    }
    l.y = bottom + CELL_PADDING;
    let y = l.y;
    l.rule(MARGIN, MARGIN + CONTENT, y, RULE, 0.75);
    for mark in &mut l.page()[first..] {
        if let Mark::Text { furniture, .. } = mark {
            *furniture = again;
        }
    }
}

/// The invoice laid out on pages.
fn lay_out(c: &Content, qr: Option<&Matrix>, fonts: &Fonts, tr: &Translator) -> Vec<Vec<Mark>> {
    let mut l = Layout::new(fonts);
    // Who sends it, to whom: two columns.
    let split = CONTENT * 0.55;
    let top = l.y;
    for (text, style) in &c.seller {
        l.block(MARGIN, split - 12.0, *style, text);
    }
    let left = l.y;
    l.y = top;
    for (text, style) in &c.buyer {
        l.block(MARGIN + split, CONTENT - split, *style, text);
    }
    l.y = l.y.max(left) + 24.0;
    l.block(MARGIN, CONTENT, TITLE, &c.title);
    l.y += 4.0;
    for fact in &c.facts {
        l.block(MARGIN, CONTENT, QUIET, fact);
    }
    l.y += 14.0;
    // The lines, the head again on each new page.
    let cols = columns(c.heads.len());
    head(&mut l, c, &cols, false);
    let (desc_x, desc_w) = cols[0];
    // What a page holds under the head: a row longer than that goes on to the next page.
    let page_room = BOTTOM - MARGIN - head_height(fonts, c, &cols);
    for row in &c.rows {
        let described = fonts.wrap(&row.description, desc_w - 2.0 * CELL_PADDING, BODY);
        let smalls: Vec<String> = row.small.iter().flat_map(|s| fonts.wrap(s, desc_w - 2.0 * CELL_PADDING, SMALL)).collect();
        let height = 2.0 * CELL_PADDING + described.len() as f32 * BODY.leading() + smalls.len() as f32 * SMALL.leading();
        // A row a page holds is never cut; a longer one starts where its first line fits.
        let needed = if height <= page_room { height } else { 2.0 * CELL_PADDING + BODY.leading() };
        if l.room(needed) {
            head(&mut l, c, &cols, true);
        }
        let top = l.y + CELL_PADDING;
        for (cell, (x, w)) in row.cells.iter().zip(&cols[1..]) {
            l.text_right(x + w - CELL_PADDING, top, BODY, cell);
        }
        let mut y = top;
        for (line, style) in described.iter().map(|t| (t, BODY)).chain(smalls.iter().map(|t| (t, SMALL))) {
            if y + style.leading() > BOTTOM {
                // Its words go on, under the head again; its figures stay on its first part.
                l.new_page();
                head(&mut l, c, &cols, true);
                y = l.y + CELL_PADDING;
            }
            l.text(desc_x + CELL_PADDING, y, style, line);
            y += style.leading();
        }
        l.y = y + CELL_PADDING;
        let bottom = l.y;
        l.rule(MARGIN, MARGIN + CONTENT, bottom, FAINT, 0.5);
    }
    // The sums, under the figures; the last one strong.
    let (last_x, last_w) = *cols.last().expect("columns");
    for (i, (label, amount)) in c.sums.iter().enumerate() {
        let style = if i + 1 == c.sums.len() { SUM_STRONG } else { SUM };
        // The amount ends at the last column's edge; one wider than its
        // column reaches left, and its label ends before it, never over it.
        let amount_left = last_x + last_w - CELL_PADDING - fonts.width(amount, style);
        let label_right = (last_x - CELL_PADDING).min(amount_left - 2.0 * CELL_PADDING);
        let labels = fonts.wrap(label, label_right - MARGIN - CELL_PADDING, style);
        if l.room(2.0 * CELL_PADDING + labels.len() as f32 * style.leading()) {
            head(&mut l, c, &cols, true);
        }
        let top = l.y + CELL_PADDING;
        let mut y = top;
        for line in &labels {
            l.text_right(label_right, y, style, line);
            y += style.leading();
        }
        l.text_right(last_x + last_w - CELL_PADDING, top, style, amount);
        l.y = y + CELL_PADDING;
        let bottom = l.y;
        l.rule(MARGIN, MARGIN + CONTENT, bottom, FAINT, 0.5);
    }
    l.y += 12.0;
    // The QR code at the right of what follows, its few words under it.
    let mut beside = 0.0;
    if let Some(matrix) = qr {
        let caption = tr.text("invoice-qr", None);
        let captions = fonts.wrap(&caption, QR_SIDE + 24.0, FOOT);
        let height = QR_SIDE + captions.len() as f32 * FOOT.leading();
        l.room(height);
        let (x, y) = (MARGIN + CONTENT - QR_SIDE, l.y);
        l.page().push(Mark::Qr { x, y, side: QR_SIDE, matrix: matrix.clone() });
        let mut top = y + QR_SIDE;
        for line in &captions {
            let width = fonts.width(line, FOOT);
            l.page().push(Mark::Text { x: x + (QR_SIDE - width) / 2.0, baseline: top + FOOT.size * 0.95, style: FOOT, text: fonts.printable(line, false), furniture: true });
            top += FOOT.leading();
        }
        beside = top;
    }
    let first = l.pages.len();
    let blocks: [(&[String], Style); 3] = [(&c.mentions, BODY), (&c.pay, BODY), (&c.notes, SMALL)];
    for (items, style) in blocks {
        if items.is_empty() {
            continue;
        }
        for item in items {
            let narrow = l.pages.len() == first && l.y < beside;
            l.block(MARGIN, if narrow { CONTENT - QR_SIDE - 14.0 } else { CONTENT }, style, item);
        }
        l.y += 8.0;
    }
    // More than one page: each says which, under its lines.
    let pages = l.pages.len();
    if pages > 1 {
        for (i, page) in l.pages.iter_mut().enumerate() {
            let text = say(tr, "invoice-page", &[("title", c.title.clone()), ("page", (i + 1).to_string()), ("pages", pages.to_string())]);
            let width = fonts.width(&text, FOOT);
            page.push(Mark::Text { x: MARGIN + CONTENT - width, baseline: HEIGHT - 28.0, style: FOOT, text: fonts.printable(&text, false), furniture: true });
        }
    }
    l.pages
}

/// What the PDF's words of `invoice` hold that its font cannot print:
/// said before issue, since a PDF/A never draws a missing glyph.
pub fn unprintable(invoice: &Invoice) -> Vec<char> {
    let Ok(fonts) = Fonts::load() else { return Vec::new() };
    let tr = Translator::new(if invoice.language.is_empty() { "en" } else { &invoice.language });
    let c = content(invoice, &tr);
    let mut out: Vec<char> = Vec::new();
    for text in c.texts() {
        for bold in [false, true] {
            for ch in fonts.missing(text, bold) {
                if !out.contains(&ch) {
                    out.push(ch);
                }
            }
        }
    }
    out
}

/// Whether `c` is written right to left (Hebrew, Arabic, Syriac, Thaana,
/// N'Ko and the scripts after them): Sioul shapes each line as one run,
/// without Unicode's bidirectional ordering, so such words would print
/// backwards, digits reversed among them.
fn right_to_left_char(c: char) -> bool {
    matches!(c, '\u{0590}'..='\u{08FF}' | '\u{FB1D}'..='\u{FDFF}' | '\u{FE70}'..='\u{FEFF}' | '\u{10800}'..='\u{10FFF}' | '\u{1E800}'..='\u{1EFFF}')
}

/// The characters of `invoice`'s page written right to left: said before
/// issue and held back, until Sioul orders such text.
pub fn right_to_left(invoice: &Invoice) -> Vec<char> {
    let tr = Translator::new(if invoice.language.is_empty() { "en" } else { &invoice.language });
    let mut out: Vec<char> = Vec::new();
    for text in content(invoice, &tr).texts() {
        for c in text.chars().filter(|c| right_to_left_char(*c)) {
            if !out.contains(&c) {
                out.push(c);
            }
        }
    }
    out
}

fn color((r, g, b): (u8, u8, u8)) -> rgb::Color {
    rgb::Color::new(r, g, b)
}

fn fill(ink: (u8, u8, u8)) -> Fill {
    Fill { paint: color(ink).into(), opacity: NormalizedF32::ONE, rule: FillRule::NonZero }
}

/// The day of issue, as the PDF's dates (its creation and the XML's).
fn issued_on(invoice: &Invoice) -> DateTime {
    let day: Date = invoice.date.get(..10).and_then(|d| d.parse().ok()).unwrap_or(Date::constant(2026, 1, 1));
    DateTime::new(day.year().unsigned_abs()).month(day.month().unsigned_abs()).day(day.day().unsigned_abs()).hour(0).minute(0).second(0).utc_offset_hour(0).utc_offset_minute(0)
}

/// The issued invoice's file: its page(s) in its language, its XML attached
/// when it has one, the QR code when `options` ask and the invoice allows.
pub fn render(invoice: &Invoice, options: &Options) -> Result<Rendered, String> {
    if invoice.older.is_some() || invoice.number.is_empty() {
        return Err("only an issued record of format 2 is written as a PDF/A".into());
    }
    let tr = Translator::new(if invoice.language.is_empty() { "en" } else { &invoice.language });
    let fonts = Fonts::load()?;
    let missing = unprintable(invoice);
    if !missing.is_empty() {
        return Err(say(&tr, "invoice-check-unprintable", &[("characters", missing.into_iter().collect())]));
    }
    let backwards = right_to_left(invoice);
    if !backwards.is_empty() {
        return Err(say(&tr, "invoice-check-right-to-left", &[("characters", backwards.into_iter().collect())]));
    }
    let c = content(invoice, &tr);
    let qr = if options.qr { epc::payload(invoice).and_then(|p| epc::matrix(&p)) } else { None };
    let pages = lay_out(&c, qr.as_ref(), &fonts, &tr);
    let (xml, without) = match cii::xml(invoice) {
        Ok(xml) => (Some(xml), None),
        Err(why) => (None, Some(why)),
    };
    let configuration = ConfigurationBuilder::new().with_archival_validator(Archival::A3_B).finish().map_err(|e| format!("{e:?}"))?;
    let settings = SerializeSettings { configuration, compress_content_streams: true, enable_tagging: false, ..SerializeSettings::default() };
    let mut document = Document::new_with(settings);
    let size = Size::from_wh(WIDTH, HEIGHT).ok_or("A4")?;
    for marks in &pages {
        let mut page = document.start_page_with(PageSettings::new(size));
        let mut surface = page.surface();
        for mark in marks {
            match mark {
                Mark::Text { x, baseline, style, text, .. } => {
                    let font = if style.bold { fonts.bold.clone() } else { fonts.regular.clone() };
                    let glyphs = fonts.glyphs(text, style.bold);
                    surface.set_fill(Some(fill(style.ink)));
                    surface.set_stroke(None);
                    surface.draw_glyphs(Point::from_xy(*x, *baseline), &glyphs, font, text, style.size, false);
                }
                Mark::Rule { x1, x2, y, ink, width } => {
                    let mut path = PathBuilder::new();
                    path.move_to(*x1, *y);
                    path.line_to(*x2, *y);
                    if let Some(path) = path.finish() {
                        surface.set_fill(None);
                        surface.set_stroke(Some(Stroke { paint: color(*ink).into(), width: *width, ..Stroke::default() }));
                        surface.draw_path(&path);
                    }
                }
                Mark::Qr { x, y, side, matrix } => {
                    // Four modules of quiet zone around the code, inside its square.
                    let module = side / (matrix.size + 8) as f32;
                    let mut path = PathBuilder::new();
                    for row in 0..matrix.size {
                        let mut column = 0;
                        while column < matrix.size {
                            if !matrix.dark(column, row) {
                                column += 1;
                                continue;
                            }
                            let start = column;
                            while column < matrix.size && matrix.dark(column, row) {
                                column += 1;
                            }
                            let left = x + (start + 4) as f32 * module;
                            let top = y + (row + 4) as f32 * module;
                            if let Some(rect) = Rect::from_xywh(left, top, (column - start) as f32 * module, module) {
                                path.push_rect(rect);
                            }
                        }
                    }
                    if let Some(path) = path.finish() {
                        surface.set_stroke(None);
                        surface.set_fill(Some(fill(BLACK)));
                        surface.draw_path(&path);
                    }
                }
            }
        }
        surface.finish();
        page.finish();
    }
    let date = issued_on(invoice);
    document.set_metadata(
        Metadata::new()
            .title(c.title.clone())
            .language(tr.language().to_string())
            .authors(vec![invoice.issuer.name.clone()])
            .creator("Sioul".into())
            .producer("Sioul".into())
            .creation_date(date),
    );
    if let Some(xml) = &xml {
        document.embed_file(EmbeddedFile {
            path: cii::FILE_NAME.into(),
            mime_type: MimeType::new("text/xml"),
            description: Some("Factur-X".into()),
            association_kind: AssociationKind::Alternative,
            data: xml.as_bytes().to_vec().into(),
            modification_date: Some(date),
            compress: Some(true),
            location: None,
        });
    }
    let pdf = document.finish().map_err(|e| format!("the invoice's PDF: {e:?}"))?;
    let pdf = if xml.is_some() { super::pdfa::with_factur_x(&pdf)? } else { pdf };
    Ok(Rendered { pdf, xml, without })
}

#[cfg(test)]
mod tests;

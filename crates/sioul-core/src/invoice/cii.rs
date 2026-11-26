// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! An issued invoice as Factur-X's XML (docs/projects.md, "The issued
//! file"): UN/CEFACT Cross Industry Invoice D22B in the EN 16931 profile
//! (`urn:cen.eu:en16931:2017`), Factur-X 1.09.2 / ZUGFeRD 2.5.2, one file
//! that serves France and Germany. Written element by element, in the order
//! the CII schema fixes, from the record's own business terms (BT-xx):
//! nothing is translated, nothing computed again.
//!
//! The VAT categories (UNTDID 5305) are the record's: S with its rate; E
//! with its reason, the French franchise as VATEX-FR-FRANCHISE and its
//! mention; AE (reverse charge) with both VAT numbers; O (not registered)
//! as VATEX-EU-O, with no VAT number on either side (BR-O-02); K (a
//! delivery to a business in another member state) as VATEX-EU-IC; G
//! (export) as VATEX-EU-G; Z (zero rated). Sources: research-generic.md §5
//! (Factur-X 1.07.2 §7, the EN 16931 rules on docs.peppol.eu, the VATEX
//! list), in docs/research/invoicing.md.
//!
//! EN 16931 asks the seller's VAT or tax number for an exempt invoice
//! (BR-E-02), and a French micro-entrepreneur under the franchise usually
//! has neither: the French rule BR-FR-CO-16 then has the SIREN given again
//! as the seller's tax registration (BT-32, `SpecifiedTaxRegistration` with
//! scheme `FC`, which BR-E-02's CII binding accepts beside `VA`), the
//! breakdown in `E` with `VATEX-FR-FRANCHISE` and the mention as its words
//! (`french_franchise`).
//!
//! Some invoices carry no XML (`without`): what the standard asks is
//! missing, and the PDF/A-3 alone is a complete invoice on paper, as French
//! law asks of small businesses until 1 September 2027.

use super::record::{Address, Invoice, Kind, Line, Nature, TaxBreak};
use crate::i18n::Translator;
use quick_xml::Writer;
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};

/// The profile: EN 16931 (BT-24).
pub const PROFILE: &str = "urn:cen.eu:en16931:2017";

/// The file's name in the PDF, as Factur-X fixes it.
pub const FILE_NAME: &str = "factur-x.xml";

/// The profile's name in the PDF's XMP (`fx:ConformanceLevel`).
pub const CONFORMANCE: &str = "EN 16931";

const RSM: &str = "urn:un:unece:uncefact:data:standard:CrossIndustryInvoice:100";
const QDT: &str = "urn:un:unece:uncefact:data:standard:QualifiedDataType:100";
const RAM: &str = "urn:un:unece:uncefact:data:standard:ReusableAggregateBusinessInformationEntity:100";
const UDT: &str = "urn:un:unece:uncefact:data:standard:UnqualifiedDataType:100";

/// Why an invoice's PDF carries no XML, said before issue as a note, never
/// as a reason not to issue: the PDF is the invoice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Without {
    /// A record from before November 2026: printed as it always was.
    Older,
    /// The seller's VAT number (or its tax number) its VAT category asks for
    /// (BR-S-02, BR-Z-02, BR-E-02, BR-AE-02, BR-IC-02, BR-G-02).
    SellerVat,
    /// The client's VAT number, for a reverse charge or a delivery to
    /// another member state (BR-AE-02, BR-IC-02).
    ClientVat,
    /// One identifier of the seller at least (BR-CO-26); under the French
    /// franchise without a VAT number, the SIREN (`french_franchise`).
    SellerId,
    /// The seller's country (BR-09).
    SellerCountry,
    /// Lines outside the scope of VAT beside others: EN 16931 has an
    /// invoice either outside it whole or not at all (BR-O-11 to 14).
    OutsideMixed,
    /// The client's or the delivery's country, not one of ISO 3166-1 (`clients::country`).
    ClientCountry,
    /// A line's price below zero (BR-27) or its unit outside Sioul's (`units`).
    Lines,
    /// A deposit invoice for goods to another member state, without the
    /// period it covers (BR-IC-11: a delivery day or a period; a deposit
    /// has no delivery yet).
    DepositPeriod,
}

impl Without {
    /// What a check says of it, in the invoice's language.
    pub fn said(&self, tr: &Translator) -> String {
        tr.text(
            match self {
                Without::Older => "invoice-xml-older",
                Without::SellerVat => "invoice-xml-seller-vat",
                Without::ClientVat => "invoice-xml-client-vat",
                Without::SellerId => "invoice-xml-seller-id",
                Without::SellerCountry => "invoice-xml-seller-country",
                Without::OutsideMixed => "invoice-xml-outside-mixed",
                Without::ClientCountry => "invoice-xml-client-country",
                Without::Lines => "invoice-xml-lines",
                Without::DepositPeriod => "invoice-xml-deposit-period",
            },
            None,
        )
    }
}

/// The identifiers the XML carries for one party.
#[derive(Debug, Default)]
struct Ids {
    /// BT-29 / BT-46, without a scheme.
    id: Option<String>,
    /// BT-30 / BT-47: a French SIREN, scheme 0002.
    legal: Option<String>,
    /// BT-31 / BT-48.
    vat: Option<String>,
    /// BT-32: a tax number that is no VAT number (Steuernummer, UTR, a tax
    /// number elsewhere); under the French franchise without a VAT number,
    /// the SIREN again (`french_franchise`).
    tax: Option<String>,
    /// Whether `tax` is that SIREN: it answers the franchise's exemption
    /// alone, never a category that asks a VAT number (`without`).
    siren_as_tax: bool,
    /// BT-33: what the trade register says (« RCS Lyon »).
    trade_register: Option<String>,
}

/// Whether the invoice is not subject to VAT: then neither side's VAT number is written (BR-O-02).
fn outside(invoice: &Invoice) -> bool {
    invoice.totals.breakdown.iter().any(|b| b.category == "O")
}

/// The French franchise's exemption code (BT-121), from the VATEX list.
const FRENCH_FRANCHISE: &str = "VATEX-FR-FRANCHISE";

/// Whether a French seller's invoice bears the franchise's exemption: a
/// breakdown in `E` with `VATEX-FR-FRANCHISE` (BT-118, BT-121), its mention
/// as its words (BT-120). Its seller without a VAT number then gives the
/// SIREN again as the seller's tax registration (BT-32): the French rule
/// BR-FR-CO-16, in our words. A credit note keeps the exemption of the
/// lines it credits (`Line::own_tax`), so it follows the same rule.
fn french_franchise(invoice: &Invoice) -> bool {
    invoice.issuer.preset == "fr" && invoice.totals.breakdown.iter().any(|b| b.category == "E" && b.reason_code.trim() == FRENCH_FRANCHISE)
}

/// The seller's identifiers, as the XML carries them.
fn seller_ids(invoice: &Invoice) -> Ids {
    let issuer = &invoice.issuer;
    let mut ids = Ids::default();
    let value = |kind: &str| issuer.ids.iter().find(|i| i.kind == kind && !i.value.trim().is_empty()).map(|i| i.value.trim().to_string());
    // A SIRET holds its SIREN: its first nine digits.
    ids.legal = value("siren").or_else(|| value("siret")).and_then(|v| siren(&v));
    let liable = issuer.scheme == "liable";
    ids.vat = value("vat").map(|v| super::ids::compact(&v)).or_else(|| if liable { value("uid").map(|v| super::ids::compact(&v)) } else { None });
    ids.tax = value("steuernummer").or_else(|| value("utr")).or_else(|| value("tax-id"));
    ids.trade_register = value("rcs").map(|city| format!("RCS {city}"));
    if outside(invoice) {
        ids.vat = None;
    }
    // The French franchise without a VAT number (BR-FR-CO-16): the SIREN
    // again as BT-32, the number EN 16931's exempt category asks (BR-E-02).
    if ids.vat.is_none() && ids.tax.is_none() && french_franchise(invoice) {
        ids.tax = ids.legal.clone();
        ids.siren_as_tax = ids.tax.is_some();
    }
    // BR-CO-26: one identifier at least; a tax number, a UID or another as the seller's own.
    if ids.legal.is_none() && ids.vat.is_none() {
        // A SIRET being given (« en cours d'attribution ») identifies no one yet.
        ids.id = ids.tax.clone().or_else(|| value("uid")).or_else(|| value("ku-idnr")).or_else(|| {
            issuer.ids.iter().find(|i| !i.value.trim().is_empty() && !super::ids::pending(&i.value) && !matches!(i.kind.as_str(), "rcs" | "vat" | "other")).map(|i| i.value.trim().to_string())
        });
    }
    ids
}

/// A SIREN, or a SIRET's first nine digits (BT-30, scheme 0002); None for
/// any other text, a SIRET being given (« en cours d’attribution ») among them.
fn siren(value: &str) -> Option<String> {
    let compact = super::ids::compact(value);
    compact.get(..9).filter(|first| first.bytes().all(|b| b.is_ascii_digit())).map(str::to_string)
}

/// The client's identifiers, as the XML carries them.
fn buyer_ids(invoice: &Invoice) -> Ids {
    let client = &invoice.client;
    let mut ids = Ids::default();
    ids.legal = client.id("siren").or_else(|| client.id("siret")).and_then(siren);
    ids.vat = if outside(invoice) { None } else { client.id("vat").map(super::ids::compact) };
    ids
}

/// Why `invoice` carries no XML, when it does not.
pub fn without(invoice: &Invoice) -> Option<Without> {
    if invoice.older.is_some() || invoice.format < 2 {
        return Some(Without::Older);
    }
    let seller = seller_ids(invoice);
    let buyer = buyer_ids(invoice);
    let categories: Vec<&str> = invoice.totals.breakdown.iter().map(|b| b.category.as_str()).collect();
    let has = |c: &str| categories.contains(&c);
    if code(&invoice.issuer.address.country).is_none() {
        return Some(Without::SellerCountry);
    }
    let client = &invoice.client;
    let delivery = client.delivery.iter().map(|d| d.country.trim()).filter(|c| !c.is_empty());
    if [client.address.country.trim()].into_iter().filter(|c| !c.is_empty()).chain(delivery).any(|c| code(c).is_none()) {
        return Some(Without::ClientCountry);
    }
    if invoice.lines.iter().any(|l| l.price < 0 || !super::units::known(&l.unit)) {
        return Some(Without::Lines);
    }
    if invoice.kind == Kind::Deposit && has("K") && invoice.period_from.is_empty() {
        return Some(Without::DepositPeriod);
    }
    if has("O") && categories.iter().any(|c| *c != "O") {
        return Some(Without::OutsideMixed);
    }
    if has("E") && seller.vat.is_none() && seller.tax.is_none() {
        // The French franchise without its SIREN either (a SIRET still being
        // given): what it lacks is that registration, not a VAT number.
        return Some(if french_franchise(invoice) { Without::SellerId } else { Without::SellerVat });
    }
    // The SIREN standing for a VAT number answers the franchise alone: a
    // reverse charge under the franchise still asks the VAT number.
    let vat_or_tax = seller.vat.is_some() || (seller.tax.is_some() && !seller.siren_as_tax);
    if (["S", "Z", "AE"].iter().any(|c| has(c)) && !vat_or_tax) || (["K", "G"].iter().any(|c| has(c)) && seller.vat.is_none()) {
        return Some(Without::SellerVat);
    }
    if (has("AE") && buyer.vat.is_none() && buyer.legal.is_none()) || (has("K") && buyer.vat.is_none()) {
        return Some(Without::ClientVat);
    }
    if seller.id.is_none() && seller.legal.is_none() && seller.vat.is_none() {
        return Some(Without::SellerId);
    }
    None
}

/// Money in cents as the XML writes it: "1234.50", "-0.05".
fn amount(cents: i64) -> String {
    let sign = if cents < 0 { "-" } else { "" };
    format!("{sign}{}.{:02}", cents.unsigned_abs() / 100, cents.unsigned_abs() % 100)
}

/// A decimal of `n` with `places` decimals, trailing zeros taken off: 1500 thousandths → "1.5".
fn decimal(n: i64, places: u32) -> String {
    let scale = 10u64.pow(places);
    let sign = if n < 0 { "-" } else { "" };
    let (whole, part) = (n.unsigned_abs() / scale, n.unsigned_abs() % scale);
    if part == 0 {
        return format!("{sign}{whole}");
    }
    let digits = format!("{part:0width$}", width = places as usize);
    format!("{sign}{whole}.{}", digits.trim_end_matches('0'))
}

/// A day as CII's format 102: "2026-11-16" → "20261116".
fn day(text: &str) -> Option<String> {
    let date: jiff::civil::Date = text.get(..10)?.parse().ok()?;
    Some(date.strftime("%Y%m%d").to_string())
}

/// Text as XML may hold it: the control characters XML 1.0 refuses taken out.
fn clean(text: &str) -> String {
    text.chars().filter(|c| !c.is_control() || matches!(c, '\n' | '\t')).collect::<String>().trim().to_string()
}

/// The reason a category without tax carries (BT-120, BT-121): the record's,
/// else the code EN 16931 asks (BR-O-10, BR-IC-10, BR-G-10, BR-AE-10).
fn reason(b: &TaxBreak) -> (String, String) {
    let code = match (b.category.as_str(), b.reason_code.trim()) {
        (_, code) if !code.is_empty() => code.to_string(),
        ("O", _) => "VATEX-EU-O".into(),
        ("K", _) => "VATEX-EU-IC".into(),
        ("G", _) => "VATEX-EU-G".into(),
        ("AE", _) => "VATEX-EU-AE".into(),
        _ => String::new(),
    };
    let text = match (b.category.as_str(), b.reason.trim()) {
        (_, text) if !text.is_empty() => clean(text),
        ("O", _) => "Not subject to VAT".into(),
        _ => String::new(),
    };
    (text, code)
}

/// A category that has no rate (BR-O-05, and its breakdown none either).
fn rated(category: &str) -> bool {
    category != "O"
}

/// The XML's writer: elements opened and closed in order, empty ones left out.
struct Out {
    w: Writer<Vec<u8>>,
    open: Vec<&'static str>,
}

type Done = std::io::Result<()>;

impl Out {
    fn open(&mut self, name: &'static str) -> Done {
        self.w.write_event(Event::Start(BytesStart::new(name)))?;
        self.open.push(name);
        Ok(())
    }

    fn close(&mut self) -> Done {
        let name = self.open.pop().unwrap_or_default();
        self.w.write_event(Event::End(BytesEnd::new(name)))
    }

    /// An element with its text; nothing when the text is empty.
    fn leaf(&mut self, name: &'static str, text: &str) -> Done {
        self.leaf_with(name, &[], text)
    }

    fn leaf_with(&mut self, name: &'static str, attributes: &[(&str, &str)], text: &str) -> Done {
        let text = clean(text);
        if text.is_empty() {
            return Ok(());
        }
        let mut start = BytesStart::new(name);
        for attribute in attributes {
            start.push_attribute(*attribute);
        }
        self.w.write_event(Event::Start(start))?;
        self.w.write_event(Event::Text(BytesText::new(&text)))?;
        self.w.write_event(Event::End(BytesEnd::new(name)))
    }

    /// A day in an element of its own: `<name><udt:DateTimeString format="102">…`.
    fn date(&mut self, name: &'static str, inner: &'static str, text: &str) -> Done {
        let Some(value) = day(text) else { return Ok(()) };
        self.open(name)?;
        self.leaf_with(inner, &[("format", "102")], &value)?;
        self.close()
    }
}

/// A postal address (BG-5, BG-8, BG-15): its lines, postcode, city, country.
fn address(out: &mut Out, address: &Address, country: &str) -> Done {
    out.open("ram:PostalTradeAddress")?;
    out.leaf("ram:PostcodeCode", &address.postcode)?;
    let lines: Vec<&str> = address.lines.iter().map(|l| l.trim()).filter(|l| !l.is_empty()).collect();
    out.leaf("ram:LineOne", lines.first().copied().unwrap_or(""))?;
    out.leaf("ram:LineTwo", lines.get(1).copied().unwrap_or(""))?;
    out.leaf("ram:LineThree", &lines.get(2..).map(|rest| rest.join(", ")).unwrap_or_default())?;
    out.leaf("ram:CityName", &address.city)?;
    out.leaf("ram:CountryID", &country.trim().to_uppercase())?;
    out.close()
}

/// A country as ISO 3166-1 writes it, from its code or its name
/// (`clients::country`: "Belgique" is BE, "UK" GB, "EL" GR); None for a
/// text that names none.
fn code(text: &str) -> Option<&'static str> {
    crate::clients::country(text).map(|c| c.code)
}

/// The country an address is in, as ISO 3166-1 writes it: its own (`without`
/// refuses one that is not a country); an address that names none is at
/// `home`.
fn country_of(address: &Address, home: &str) -> String {
    let own = address.country.trim();
    code(if own.is_empty() { home } else { own }).unwrap_or_default().to_string()
}

/// France's business process (BT-23): goods, services or both (B1, S1, M1),
/// as the reform's data table asks (research-generic.md §5).
fn process(invoice: &Invoice) -> Option<&'static str> {
    if invoice.issuer.preset != "fr" {
        return None;
    }
    let goods = invoice.lines.iter().any(|l| l.nature == Nature::Goods);
    let services = invoice.lines.iter().any(|l| l.nature == Nature::Service);
    Some(match (goods, services) {
        (true, true) => "M1",
        (true, false) => "B1",
        _ => "S1",
    })
}

/// One line (BG-25).
fn line(out: &mut Out, n: usize, line: &Line, tr: &Translator) -> Done {
    out.open("ram:IncludedSupplyChainTradeLineItem")?;
    out.open("ram:AssociatedDocumentLineDocument")?;
    out.leaf("ram:LineID", &n.to_string())?;
    out.close()?;
    out.open("ram:SpecifiedTradeProduct")?;
    out.leaf("ram:Name", &line.description)?;
    out.leaf("ram:Description", &line.details)?;
    out.close()?;
    out.open("ram:SpecifiedLineTradeAgreement")?;
    out.open("ram:NetPriceProductTradePrice")?;
    out.leaf("ram:ChargeAmount", &amount(line.price))?;
    if line.base != 1 {
        out.leaf_with("ram:BasisQuantity", &[("unitCode", &line.unit)], &line.base.to_string())?;
    }
    out.close()?;
    out.close()?;
    out.open("ram:SpecifiedLineTradeDelivery")?;
    out.leaf_with("ram:BilledQuantity", &[("unitCode", &line.unit)], &decimal(line.quantity, 3))?;
    out.close()?;
    out.open("ram:SpecifiedLineTradeSettlement")?;
    out.open("ram:ApplicableTradeTax")?;
    out.leaf("ram:TypeCode", "VAT")?;
    out.leaf("ram:CategoryCode", &line.category)?;
    if rated(&line.category) {
        out.leaf("ram:RateApplicablePercent", &decimal(line.rate, 2))?;
    }
    out.close()?;
    if line.discount != 0 {
        out.open("ram:SpecifiedTradeAllowanceCharge")?;
        out.open("ram:ChargeIndicator")?;
        out.leaf("udt:Indicator", "false")?;
        out.close()?;
        out.leaf("ram:ActualAmount", &amount(line.discount))?;
        // UNTDID 5189: 95, a discount (BR-42 asks a reason or its code).
        out.leaf("ram:ReasonCode", "95")?;
        out.leaf("ram:Reason", &tr.text("invoice-xml-discount", None))?;
        out.close()?;
    }
    out.open("ram:SpecifiedTradeSettlementLineMonetarySummation")?;
    out.leaf("ram:LineTotalAmount", &amount(line.amount))?;
    out.close()?;
    out.close()?;
    out.close()
}

/// A party: its own identifier, name, register, legal registration, a
/// person to write to, address, mail, VAT and tax numbers.
#[allow(clippy::too_many_arguments)]
fn party(out: &mut Out, element: &'static str, ids: &Ids, name: &str, trade_name: &str, person: &str, place: &Address, country: &str, email: &str) -> Done {
    out.open(element)?;
    out.leaf("ram:ID", ids.id.as_deref().unwrap_or(""))?;
    out.leaf("ram:Name", name)?;
    out.leaf("ram:Description", ids.trade_register.as_deref().unwrap_or(""))?;
    if ids.legal.is_some() || !trade_name.trim().is_empty() {
        out.open("ram:SpecifiedLegalOrganization")?;
        if let Some(siren) = &ids.legal {
            out.leaf_with("ram:ID", &[("schemeID", "0002")], siren)?;
        }
        out.leaf("ram:TradingBusinessName", trade_name)?;
        out.close()?;
    }
    if !person.trim().is_empty() {
        out.open("ram:DefinedTradeContact")?;
        out.leaf("ram:PersonName", person)?;
        out.close()?;
    }
    address(out, place, country)?;
    if !email.trim().is_empty() {
        out.open("ram:URIUniversalCommunication")?;
        out.leaf_with("ram:URIID", &[("schemeID", "EM")], email)?;
        out.close()?;
    }
    for (value, scheme) in [(&ids.vat, "VA"), (&ids.tax, "FC")] {
        if let Some(value) = value {
            out.open("ram:SpecifiedTaxRegistration")?;
            out.leaf_with("ram:ID", &[("schemeID", scheme)], value)?;
            out.close()?;
        }
    }
    out.close()
}

/// The invoice's XML, or why it has none (`without`).
pub fn xml(invoice: &Invoice) -> Result<String, Without> {
    if let Some(why) = without(invoice) {
        return Err(why);
    }
    write(invoice).map_err(|_| Without::Older)
}

fn write(invoice: &Invoice) -> std::io::Result<String> {
    let tr = Translator::new(if invoice.language.is_empty() { "en" } else { &invoice.language });
    let mut out = Out { w: Writer::new_with_indent(Vec::new(), b' ', 2), open: Vec::new() };
    out.w.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;
    let mut root = BytesStart::new("rsm:CrossIndustryInvoice");
    root.push_attribute(("xmlns:rsm", RSM));
    root.push_attribute(("xmlns:qdt", QDT));
    root.push_attribute(("xmlns:ram", RAM));
    root.push_attribute(("xmlns:udt", UDT));
    out.w.write_event(Event::Start(root))?;
    out.open.push("rsm:CrossIndustryInvoice");

    out.open("rsm:ExchangedDocumentContext")?;
    if let Some(code) = process(invoice) {
        out.open("ram:BusinessProcessSpecifiedDocumentContextParameter")?;
        out.leaf("ram:ID", code)?;
        out.close()?;
    }
    out.open("ram:GuidelineSpecifiedDocumentContextParameter")?;
    out.leaf("ram:ID", PROFILE)?;
    out.close()?;
    out.close()?;

    out.open("rsm:ExchangedDocument")?;
    out.leaf("ram:ID", &invoice.number)?;
    out.leaf("ram:TypeCode", &invoice.kind.code().to_string())?;
    out.date("ram:IssueDateTime", "udt:DateTimeString", &invoice.date)?;
    let mut notes: Vec<(String, String)> = invoice.notes.iter().filter(|n| !n.text.trim().is_empty()).map(|n| (n.text.clone(), n.subject.clone())).collect();
    if !invoice.issuer.insurance.trim().is_empty() {
        let mut args = crate::i18n::args();
        args.set("insurance", invoice.issuer.insurance.clone());
        notes.push((tr.text("invoice-insurance", Some(&args)), String::new()));
    }
    for (text, subject) in &notes {
        out.open("ram:IncludedNote")?;
        out.leaf("ram:Content", text)?;
        out.leaf("ram:SubjectCode", subject)?;
        out.close()?;
    }
    out.close()?;

    out.open("rsm:SupplyChainTradeTransaction")?;
    for (n, l) in invoice.lines.iter().enumerate() {
        line(&mut out, n + 1, l, &tr)?;
    }

    let home = code(&invoice.issuer.address.country).unwrap_or_default().to_string();
    out.open("ram:ApplicableHeaderTradeAgreement")?;
    let issuer = &invoice.issuer;
    party(&mut out, "ram:SellerTradeParty", &seller_ids(invoice), &issuer.name, &issuer.trade_name, "", &issuer.address, &home, &issuer.email)?;
    let client = &invoice.client;
    let mut names = client.name.lines().map(str::trim).filter(|l| !l.is_empty());
    let first = names.next().unwrap_or("");
    let person = names.collect::<Vec<_>>().join(", ");
    // The client's country as the VAT's place reads it (`life::client_country`:
    // its address's, else its VAT number's); an address naming none is at
    // home, as the page prints it, the seller's.
    let theirs = match super::life::client_country(client) {
        found if found.is_empty() => home.clone(),
        found => code(&found).unwrap_or_default().to_string(),
    };
    party(&mut out, "ram:BuyerTradeParty", &buyer_ids(invoice), first, "", &person, &client.address, &theirs, &client.email)?;
    if !invoice.order.trim().is_empty() {
        out.open("ram:BuyerOrderReferencedDocument")?;
        out.leaf("ram:IssuerAssignedID", &invoice.order)?;
        out.close()?;
    }
    out.close()?;

    // Where goods went (BG-13): a delivery address apart, or the client's
    // for a delivery to another member state (BR-IC-12); the day the supply
    // was completed (BT-72, BR-IC-11): the last day of its period, else the
    // day of issue, as the page says by saying no other.
    let intra = invoice.totals.breakdown.iter().any(|b| b.category == "K");
    out.open("ram:ApplicableHeaderTradeDelivery")?;
    let delivery = client.delivery.as_ref().filter(|d| !d.is_empty() && **d != client.address);
    if let Some(place) = delivery.or(intra.then_some(&client.address)) {
        out.open("ram:ShipToTradeParty")?;
        address(&mut out, place, &country_of(place, &theirs))?;
        out.close()?;
    }
    // A deposit invoice bills what is not supplied yet: no such day (its
    // period, when it says one, is BG-14 below).
    let supplied = if invoice.kind == Kind::Deposit { "" } else { [invoice.period_to.as_str(), invoice.period_from.as_str(), invoice.date.as_str()].into_iter().find(|d| day(d).is_some()).unwrap_or("") };
    if !supplied.is_empty() {
        out.open("ram:ActualDeliverySupplyChainEvent")?;
        out.date("ram:OccurrenceDateTime", "udt:DateTimeString", supplied)?;
        out.close()?;
    }
    out.close()?;

    out.open("ram:ApplicableHeaderTradeSettlement")?;
    // The account the QR code names too: the first whose IBAN's key holds, else the first.
    let bank = super::epc::account(issuer).filter(|_| invoice.kind != Kind::CreditNote);
    if bank.is_some() {
        out.leaf("ram:PaymentReference", &invoice.number)?;
    }
    out.leaf("ram:InvoiceCurrencyCode", &invoice.currency.trim().to_uppercase())?;
    if let Some(bank) = bank {
        let account = super::ids::compact(&bank.iban);
        let iban = super::epc::iban_valid(&account);
        out.open("ram:SpecifiedTradeSettlementPaymentMeans")?;
        // UNTDID 4461: 58, a SEPA credit transfer; 30, any other.
        out.leaf("ram:TypeCode", if iban && invoice.currency.eq_ignore_ascii_case("EUR") { "58" } else { "30" })?;
        // The schema's order: IBANID, AccountName, ProprietaryID.
        out.open("ram:PayeePartyCreditorFinancialAccount")?;
        if iban {
            out.leaf("ram:IBANID", &account)?;
        }
        out.leaf("ram:AccountName", &bank.holder)?;
        if !iban {
            out.leaf("ram:ProprietaryID", &account)?;
        }
        out.close()?;
        if !bank.bic.trim().is_empty() {
            out.open("ram:PayeeSpecifiedCreditorFinancialInstitution")?;
            out.leaf("ram:BICID", &super::ids::compact(&bank.bic))?;
            out.close()?;
        }
        out.close()?;
    }
    for b in &invoice.totals.breakdown {
        let (text, code) = reason(b);
        out.open("ram:ApplicableTradeTax")?;
        out.leaf("ram:CalculatedAmount", &amount(b.tax))?;
        out.leaf("ram:TypeCode", "VAT")?;
        if !matches!(b.category.as_str(), "S" | "Z") {
            out.leaf("ram:ExemptionReason", &text)?;
        }
        out.leaf("ram:BasisAmount", &amount(b.base))?;
        out.leaf("ram:CategoryCode", &b.category)?;
        if !matches!(b.category.as_str(), "S" | "Z") {
            out.leaf("ram:ExemptionReasonCode", &code)?;
        }
        if rated(&b.category) {
            out.leaf("ram:RateApplicablePercent", &decimal(b.rate, 2))?;
        }
        out.close()?;
    }
    if !invoice.period_from.is_empty() {
        let to = if invoice.period_to.is_empty() { &invoice.period_from } else { &invoice.period_to };
        out.open("ram:BillingSpecifiedPeriod")?;
        out.date("ram:StartDateTime", "udt:DateTimeString", &invoice.period_from)?;
        out.date("ram:EndDateTime", "udt:DateTimeString", to)?;
        out.close()?;
    }
    let terms = if invoice.kind == Kind::CreditNote { "" } else { issuer.payment.as_str() };
    if !terms.trim().is_empty() || day(&invoice.due).is_some() {
        out.open("ram:SpecifiedTradePaymentTerms")?;
        out.leaf("ram:Description", terms)?;
        out.date("ram:DueDateDateTime", "udt:DateTimeString", &invoice.due)?;
        out.close()?;
    }
    let t = &invoice.totals;
    let currency = invoice.currency.trim().to_uppercase();
    out.open("ram:SpecifiedTradeSettlementHeaderMonetarySummation")?;
    out.leaf("ram:LineTotalAmount", &amount(t.lines))?;
    out.leaf("ram:TaxBasisTotalAmount", &amount(t.without_tax))?;
    out.leaf_with("ram:TaxTotalAmount", &[("currencyID", &currency)], &amount(t.tax))?;
    out.leaf("ram:GrandTotalAmount", &amount(t.with_tax))?;
    if t.deposits != 0 {
        out.leaf("ram:TotalPrepaidAmount", &amount(t.deposits))?;
    }
    out.leaf("ram:DuePayableAmount", &amount(t.due))?;
    out.close()?;
    // The invoices it cites (BG-3, as many as the profile's schema takes): the
    // one a credit note corrects, the deposit invoices a final one deducts.
    for cited in invoice.corrects.iter().chain(&invoice.deposits).filter(|c| !c.number.is_empty()) {
        out.open("ram:InvoiceReferencedDocument")?;
        out.leaf("ram:IssuerAssignedID", &cited.number)?;
        out.date("ram:FormattedIssueDateTime", "qdt:DateTimeString", &cited.date)?;
        out.close()?;
    }
    out.close()?;
    out.close()?;
    out.close()?;
    String::from_utf8(out.w.into_inner()).map_err(std::io::Error::other)
}

#[cfg(test)]
mod tests;

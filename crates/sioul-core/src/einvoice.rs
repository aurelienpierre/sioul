// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Invoices that carry their own data (docs/accounting.md, "Matching"):
//! Factur-X (ZUGFeRD), a PDF carrying its invoice as UN/CEFACT CII XML, and
//! UBL 2.1, the two forms of France's electronic invoices (every business
//! receives them since 1 September 2026; small ones issue them from 1
//! September 2027: impots.gouv.fr). What they say is the invoice's own
//! word: its number, its amount due, its day due, who sends it, the payment
//! reference and the SEPA mandate a bank's line will carry. Read for the
//! matching's keys (`matching`), never shown whole: an IBAN is kept to tell
//! a payment, not to display.
//!
//! Only what is needed is read: the embedded file of a PDF, found by its
//! stream's `/Type /EmbeddedFile`, inflated when Flate-compressed (64 MiB at
//! most, as the sharing reads a sealed file), then its XML by its elements'
//! local names, namespaces aside.

use crate::money::Money;
use jiff::civil::Date;

/// What an electronic invoice says.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EInvoice {
    /// Its number (CII `ExchangedDocument/ID`, UBL `ID`).
    pub number: String,
    pub issued: Option<Date>,
    pub due: Option<Date>,
    /// Who sends it, and to whom.
    pub seller: String,
    pub buyer: String,
    /// What is due (CII `DuePayableAmount`, else `GrandTotalAmount`; UBL `PayableAmount`).
    pub amount: Option<Money>,
    /// ISO 4217; "EUR" when it says none.
    pub currency: String,
    /// The reference a transfer for it should carry (CII `PaymentReference`, UBL `PaymentID`).
    pub payment_reference: String,
    /// The SEPA mandate a direct debit of it carries (RUM), and the creditor's identifier (ICS).
    pub mandate: String,
    pub creditor: String,
    /// The account it is to be paid to.
    pub iban: String,
}

impl EInvoice {
    /// Its references, as the matching compares them (`matching::references`).
    pub fn references(&self) -> Vec<String> {
        let compact = |s: &str| s.chars().filter(char::is_ascii_alphanumeric).collect::<String>().to_uppercase();
        let mut out: Vec<String> = [("ref", &self.number), ("ref", &self.payment_reference), ("rum", &self.mandate), ("ics", &self.creditor)]
            .into_iter()
            .map(|(kind, value)| (kind, compact(value)))
            .filter(|(_, value)| value.len() >= 4)
            .map(|(kind, value)| format!("{kind}:{value}"))
            .collect();
        out.sort();
        out.dedup();
        out
    }
}

/// The largest embedded file inflated.
const LARGEST: u64 = 64 * 1024 * 1024;

/// The invoice a Factur-X PDF carries, from the first of its embedded files
/// that is an invoice's XML; none when it carries none.
pub fn from_pdf(bytes: &[u8]) -> Option<EInvoice> {
    embedded_files(bytes).into_iter().find_map(|xml| from_xml(&xml))
}

/// The invoice an XML text is: CII (`CrossIndustryInvoice`) or UBL (`Invoice`, `CreditNote`).
pub fn from_xml(text: &str) -> Option<EInvoice> {
    let doc = roxmltree::Document::parse(text.trim_start_matches('\u{feff}')).ok()?;
    let root = doc.root_element();
    match root.tag_name().name() {
        "CrossIndustryInvoice" => Some(cii(root)),
        "Invoice" | "CreditNote" => Some(ubl(root)),
        _ => None,
    }
}

type Node<'a, 'input> = roxmltree::Node<'a, 'input>;

/// The element down `names`, by local names.
fn at<'a, 'input>(node: Node<'a, 'input>, names: &[&str]) -> Option<Node<'a, 'input>> {
    names.iter().try_fold(node, |n, name| n.children().find(|c| c.is_element() && c.tag_name().name() == *name))
}

fn text(node: Node, names: &[&str]) -> String {
    at(node, names).and_then(|n| n.text()).map(|t| t.trim().to_string()).unwrap_or_default()
}

/// "20261105" (CII's format 102) or "2026-11-05".
fn day(value: &str) -> Option<Date> {
    let v = value.trim();
    if v.len() == 8 && v.chars().all(|c| c.is_ascii_digit()) {
        return Date::new(v[..4].parse().ok()?, v[4..6].parse().ok()?, v[6..8].parse().ok()?).ok();
    }
    v.get(..10)?.parse().ok()
}

fn money(value: &str) -> Option<Money> {
    if value.trim().is_empty() { None } else { crate::bank::parse_amount(value) }
}

/// UN/CEFACT Cross Industry Invoice, as Factur-X and ZUGFeRD carry it (EN 16931).
fn cii(root: Node) -> EInvoice {
    let trade = at(root, &["SupplyChainTradeTransaction"]);
    let agreement = trade.and_then(|t| at(t, &["ApplicableHeaderTradeAgreement"]));
    let settlement = trade.and_then(|t| at(t, &["ApplicableHeaderTradeSettlement"]));
    let of = |node: Option<Node>, names: &[&str]| node.map(|n| text(n, names)).unwrap_or_default();
    let summation = &["SpecifiedTradeSettlementHeaderMonetarySummation"];
    let total = |name: &str| settlement.and_then(|s| at(s, summation)).map(|s| text(s, &[name])).and_then(|t| money(&t));
    EInvoice {
        number: text(root, &["ExchangedDocument", "ID"]),
        issued: day(&text(root, &["ExchangedDocument", "IssueDateTime", "DateTimeString"])),
        due: day(&of(settlement, &["SpecifiedTradePaymentTerms", "DueDateDateTime", "DateTimeString"])),
        seller: of(agreement, &["SellerTradeParty", "Name"]),
        buyer: of(agreement, &["BuyerTradeParty", "Name"]),
        amount: total("DuePayableAmount").or_else(|| total("GrandTotalAmount")),
        currency: Some(of(settlement, &["InvoiceCurrencyCode"])).filter(|c| !c.is_empty()).unwrap_or_else(|| "EUR".into()),
        payment_reference: of(settlement, &["PaymentReference"]),
        mandate: of(settlement, &["SpecifiedTradePaymentTerms", "DirectDebitMandateID"]),
        creditor: of(settlement, &["CreditorReferenceID"]),
        iban: of(settlement, &["SpecifiedTradeSettlementPaymentMeans", "PayeePartyCreditorFinancialAccount", "IBANID"]),
    }
}

/// OASIS UBL 2.1, an invoice or a credit note.
fn ubl(root: Node) -> EInvoice {
    let party_name = |party: &str| {
        let party = at(root, &[party, "Party"]);
        party.map(|p| text(p, &["PartyName", "Name"])).filter(|n| !n.is_empty()).or_else(|| party.map(|p| text(p, &["PartyLegalEntity", "RegistrationName"]))).unwrap_or_default()
    };
    let means = at(root, &["PaymentMeans"]);
    // The creditor's identifier (EN 16931, BT-90): a party identification of scheme "SEPA".
    let creditor = [&["AccountingSupplierParty", "Party"][..], &["PayeeParty"][..]]
        .iter()
        .filter_map(|path| at(root, path))
        .flat_map(|party| party.children().filter(|c| c.tag_name().name() == "PartyIdentification"))
        .filter_map(|id| at(id, &["ID"]))
        .find(|id| id.attribute("schemeID").is_some_and(|s| s.eq_ignore_ascii_case("SEPA")))
        .and_then(|id| id.text())
        .map(|t| t.trim().to_string())
        .unwrap_or_default();
    EInvoice {
        number: text(root, &["ID"]),
        issued: day(&text(root, &["IssueDate"])),
        due: day(&text(root, &["DueDate"])).or_else(|| means.and_then(|m| day(&text(m, &["PaymentDueDate"])))),
        seller: party_name("AccountingSupplierParty"),
        buyer: party_name("AccountingCustomerParty"),
        amount: money(&text(root, &["LegalMonetaryTotal", "PayableAmount"])),
        currency: Some(text(root, &["DocumentCurrencyCode"])).filter(|c| !c.is_empty()).unwrap_or_else(|| "EUR".into()),
        payment_reference: means.map(|m| text(m, &["PaymentID"])).unwrap_or_default(),
        mandate: means.map(|m| text(m, &["PaymentMandate", "ID"])).unwrap_or_default(),
        creditor,
        iban: means.map(|m| text(m, &["PayeeFinancialAccount", "ID"])).unwrap_or_default(),
    }
}

/// The embedded files of a PDF, as text: each stream whose dictionary says
/// `/Type /EmbeddedFile`, inflated when its filter is `/FlateDecode`.
fn embedded_files(pdf: &[u8]) -> Vec<String> {
    let find = |hay: &[u8], needle: &[u8], from: usize| hay.get(from..).and_then(|rest| rest.windows(needle.len()).position(|w| w == needle)).map(|at| at + from);
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(start) = find(pdf, b"stream", from) {
        from = start + 6;
        // "endstream" is no stream's start.
        if start >= 3 && &pdf[start - 3..start] == b"end" {
            continue;
        }
        // The stream's dictionary: from its object's "obj" to the keyword.
        let head_from = start.saturating_sub(4096);
        let head = &pdf[head_from..start];
        let Some(obj) = head.windows(3).rposition(|w| w == b"obj") else { continue };
        let dictionary = String::from_utf8_lossy(&head[obj + 3..]);
        let compact: String = dictionary.chars().filter(|c| !c.is_whitespace()).collect();
        if !compact.contains("/Type/EmbeddedFile") {
            continue;
        }
        let mut data_start = start + 6;
        if pdf.get(data_start) == Some(&b'\r') {
            data_start += 1;
        }
        if pdf.get(data_start) == Some(&b'\n') {
            data_start += 1;
        }
        let Some(end) = find(pdf, b"endstream", data_start) else { break };
        let mut data = &pdf[data_start..end];
        while let [rest @ .., b'\r' | b'\n'] = data {
            data = rest;
        }
        let bytes = if compact.contains("/FlateDecode") {
            use std::io::Read;
            let mut inflated = Vec::new();
            match flate2::read::ZlibDecoder::new(data).take(LARGEST).read_to_end(&mut inflated) {
                Ok(_) => inflated,
                Err(_) => continue,
            }
        } else {
            data.to_vec()
        };
        if let Ok(text) = String::from_utf8(bytes) {
            out.push(text);
        }
        from = end + 9;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const CII: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<rsm:CrossIndustryInvoice xmlns:rsm="urn:un:unece:uncefact:data:standard:CrossIndustryInvoice:100" xmlns:ram="urn:un:unece:uncefact:data:standard:ReusableAggregateBusinessInformationEntity:100" xmlns:udt="urn:un:unece:uncefact:data:standard:UnqualifiedDataType:100">
  <rsm:ExchangedDocument>
    <ram:ID>FA-2026-0042</ram:ID>
    <ram:IssueDateTime><udt:DateTimeString format="102">20261102</udt:DateTimeString></ram:IssueDateTime>
  </rsm:ExchangedDocument>
  <rsm:SupplyChainTradeTransaction>
    <ram:ApplicableHeaderTradeAgreement>
      <ram:SellerTradeParty><ram:Name>Énergie Exemple</ram:Name></ram:SellerTradeParty>
      <ram:BuyerTradeParty><ram:Name>Camille Exemple</ram:Name></ram:BuyerTradeParty>
    </ram:ApplicableHeaderTradeAgreement>
    <ram:ApplicableHeaderTradeSettlement>
      <ram:CreditorReferenceID>FR12ZZZ123456</ram:CreditorReferenceID>
      <ram:PaymentReference>CLT-778899</ram:PaymentReference>
      <ram:InvoiceCurrencyCode>EUR</ram:InvoiceCurrencyCode>
      <ram:SpecifiedTradeSettlementPaymentMeans><ram:PayeePartyCreditorFinancialAccount><ram:IBANID>FR7600000000000000000000000</ram:IBANID></ram:PayeePartyCreditorFinancialAccount></ram:SpecifiedTradeSettlementPaymentMeans>
      <ram:SpecifiedTradePaymentTerms>
        <ram:DueDateDateTime><udt:DateTimeString format="102">20261120</udt:DateTimeString></ram:DueDateDateTime>
        <ram:DirectDebitMandateID>RUM-EX-0001</ram:DirectDebitMandateID>
      </ram:SpecifiedTradePaymentTerms>
      <ram:SpecifiedTradeSettlementHeaderMonetarySummation>
        <ram:GrandTotalAmount>84.20</ram:GrandTotalAmount>
        <ram:DuePayableAmount>84.20</ram:DuePayableAmount>
      </ram:SpecifiedTradeSettlementHeaderMonetarySummation>
    </ram:ApplicableHeaderTradeSettlement>
  </rsm:SupplyChainTradeTransaction>
</rsm:CrossIndustryInvoice>"#;

    /// A PDF holding `xml` as an embedded file, Flate-compressed, as a Factur-X writer makes it.
    fn pdf_with(xml: &str) -> Vec<u8> {
        use std::io::Write;
        let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        z.write_all(xml.as_bytes()).unwrap();
        let data = z.finish().unwrap();
        let mut pdf = b"%PDF-1.7\n1 0 obj\n<< /Type /Catalog >>\nendobj\n5 0 obj\n<< /Type /EmbeddedFile /Subtype /text#2Fxml /Filter /FlateDecode /Length ".to_vec();
        pdf.extend(format!("{} >>\nstream\r\n", data.len()).bytes());
        pdf.extend(&data);
        pdf.extend(b"\r\nendstream\nendobj\ntrailer\n<< /Root 1 0 R >>\n%%EOF\n");
        pdf
    }

    /// A Factur-X invoice: its number, amounts, days, parties and the keys a bank's line will carry.
    #[test]
    fn a_factur_x_pdf_says_its_invoice() {
        let invoice = from_pdf(&pdf_with(CII)).expect("an invoice");
        assert_eq!((invoice.number.as_str(), invoice.amount, invoice.due, invoice.issued), ("FA-2026-0042", Some(Money(8420)), "2026-11-20".parse().ok(), "2026-11-02".parse().ok()));
        assert_eq!((invoice.seller.as_str(), invoice.currency.as_str()), ("Énergie Exemple", "EUR"));
        assert_eq!(invoice.references(), vec!["ics:FR12ZZZ123456", "ref:CLT778899", "ref:FA20260042", "rum:RUMEX0001"]);
        // A PDF with nothing embedded, or something else embedded: none.
        assert_eq!(from_pdf(b"%PDF-1.4\n1 0 obj\n<< /Type /Catalog >>\nendobj\n"), None);
        assert_eq!(from_pdf(&pdf_with("<note>hello</note>")), None);
    }

    #[test]
    fn a_ubl_invoice_says_the_same() {
        let ubl = r#"<Invoice xmlns="urn:oasis:names:specification:ubl:schema:xsd:Invoice-2" xmlns:cac="urn:oasis:names:specification:ubl:schema:xsd:CommonAggregateComponents-2" xmlns:cbc="urn:oasis:names:specification:ubl:schema:xsd:CommonBasicComponents-2">
  <cbc:ID>INV-7781</cbc:ID><cbc:IssueDate>2026-11-01</cbc:IssueDate><cbc:DueDate>2026-12-01</cbc:DueDate><cbc:DocumentCurrencyCode>EUR</cbc:DocumentCurrencyCode>
  <cac:AccountingSupplierParty><cac:Party><cac:PartyIdentification><cbc:ID schemeID="SEPA">FR99ZZZ654321</cbc:ID></cac:PartyIdentification><cac:PartyName><cbc:Name>Hébergeur Exemple</cbc:Name></cac:PartyName></cac:Party></cac:AccountingSupplierParty>
  <cac:PaymentMeans><cbc:PaymentMeansCode>58</cbc:PaymentMeansCode><cbc:PaymentID>HOST-2026-11</cbc:PaymentID></cac:PaymentMeans>
  <cac:LegalMonetaryTotal><cbc:PayableAmount currencyID="EUR">12.00</cbc:PayableAmount></cac:LegalMonetaryTotal>
</Invoice>"#;
        let invoice = from_xml(ubl).unwrap();
        assert_eq!((invoice.number.as_str(), invoice.amount, invoice.due, invoice.seller.as_str(), invoice.creditor.as_str()), ("INV-7781", Some(Money(1200)), "2026-12-01".parse().ok(), "Hébergeur Exemple", "FR99ZZZ654321"));
        assert!(invoice.references().contains(&"ref:HOST202611".to_string()));
    }
}

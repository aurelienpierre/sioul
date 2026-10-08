// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Mail about money: a payment made or received, a bill, an order, a refund.
//!
//! Read from the subject, then the start of the text, in French and English:
//! the payment is stated at the top, and footers say things like "vous avez
//! reçu ce message". A detection only proposes a budget line; nothing counts
//! until a rule, or you, says so (docs/accounting.md). Newsletters, activity
//! reports and shipping notices are not payments. The words looked for are
//! the `payments` and `money` lists of the word packs in use, with your
//! changes (`words::Payments`, docs/words.md).

use crate::card::Card;
use crate::money::{self, Amount};
use crate::text::{find_word, fold};
use crate::words::Words;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PaymentKind {
    /// Money that came in: a payment, a donation, a payout.
    Received,
    /// A payment you made: a receipt, a debit, a bill settled.
    Paid,
    /// A merchant's order confirmation: often the same payment a processor reports.
    Order,
    /// A bill to pay.
    Bill,
    /// Money given back.
    Refund,
}

impl PaymentKind {
    /// Money in or out.
    pub fn is_credit(self) -> bool {
        matches!(self, PaymentKind::Received | PaymentKind::Refund)
    }

    /// The word for it, as an id the translator knows ("kind-received").
    pub fn message_id(self) -> &'static str {
        match self {
            PaymentKind::Received => "kind-received",
            PaymentKind::Paid => "kind-paid",
            PaymentKind::Order => "kind-order",
            PaymentKind::Bill => "kind-bill",
            PaymentKind::Refund => "kind-refund",
        }
    }
}

/// What a message says about money.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Payment {
    pub kind: PaymentKind,
    /// Without sign: the kind gives the direction. None when the message gives no amount
    /// (a bill to read in a customer space).
    pub amount: Option<Amount>,
    /// Who paid or was paid, as the message names them; else the sender.
    pub party: String,
}

/// Characters of the text read: the payment is stated at the top.
const READ: usize = 2500;

/// What a message says about money, if it says something: subjects about
/// money that are no payment left out (`Payments::not_payments`), then the
/// phrases of each kind, tried in this order: a refund, a payment made,
/// money received, an order, a bill (a receipt "pour votre paiement" must not
/// be read as a payment received, nor the settlement of a bill as the bill).
pub fn detect(words: &Words, card: &Card) -> Option<Payment> {
    if card.is_list {
        return None;
    }
    let subject = fold(&card.subject);
    if words.payments.not_payments.iter().any(|p| find_word(&subject, p, 0).is_some()) {
        return None;
    }
    let start: String = card.excerpt.chars().take(READ).collect();
    let kind = kind_in(words, &subject).or_else(|| order_subject(words, &subject)).or_else(|| kind_in(words, &fold(&start)))?;
    Some(read(words, card, kind, &start))
}

/// The amount and the party, for a message known to be about money (a rule says so).
/// The party comes from the sentence stating the payment, else from a receipt's
/// label ("Paiement à" and the name under it), else it is the sender.
pub fn read(words: &Words, card: &Card, kind: PaymentKind, start: &str) -> Payment {
    let found = money::payment_amount(&words.money, &card.subject, start);
    let party = money::stated_amount(&words.money, start)
        .and_then(|f| party(&words.payments.leads, &f.after))
        .or_else(|| labelled_party(words, start, kind))
        .unwrap_or_else(|| card.sender().to_string());
    let amount = found.map(|f| Amount { money: f.amount.money.abs(), currency: f.amount.currency });
    Payment { kind, amount, party }
}

/// Receipts set who was paid, or who paid, under a label: "Paiement à" then "Exemple Web".
fn labelled_party(words: &Words, text: &str, kind: PaymentKind) -> Option<String> {
    let labels: Vec<String> = if kind.is_credit() { &words.payments.from } else { &words.payments.to }.iter().map(|w| crate::words::folded(w)).collect();
    let lines: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    lines.windows(2).find_map(|pair| {
        let label: String = fold(pair[0]).into_iter().collect();
        let label = label.trim_end_matches([':', ' ', '\u{a0}', '\u{202f}']);
        let name = pair[1];
        (labels.iter().any(|l| l == label) && name.chars().count() <= 60 && !name.contains('@')).then(|| name.to_string())
    })
}

/// The start of a message's text, as `detect` reads it.
pub fn start(card: &Card) -> String {
    card.excerpt.chars().take(READ).collect()
}

fn kind_in(words: &Words, folded: &[char]) -> Option<PaymentKind> {
    let p = &words.payments;
    [(PaymentKind::Refund, &p.refund), (PaymentKind::Paid, &p.paid), (PaymentKind::Received, &p.received), (PaymentKind::Order, &p.order), (PaymentKind::Bill, &p.bill)]
        .into_iter()
        .find(|(_, phrases)| phrases.iter().any(|w| find_word(folded, w, 0).is_some()))
        .map(|(kind, _)| kind)
}

/// "Commande FR000000 confirmée": the order and its confirmation, apart.
fn order_subject(words: &Words, folded: &[char]) -> Option<PaymentKind> {
    let said = |list: &[String]| list.iter().any(|w| find_word(folded, w, 0).is_some());
    (said(&words.payments.order_words) && said(&words.payments.confirmed_words)).then_some(PaymentKind::Order)
}

/// Who the payment went to or came from, in what follows its amount: "à Exemple
/// Retail.", "en faveur de SHOP", "de Jean Exemple (jean@example.org)".
/// `leads`: the words before the name (`Payments::leads`), the longest tried
/// first ("de la part de" before "de"), a space after each.
fn party(leads: &[String], after: &str) -> Option<String> {
    let mut leads: Vec<&str> = leads.iter().map(|l| l.trim()).filter(|l| !l.is_empty()).collect();
    leads.sort_by_key(|l| std::cmp::Reverse(l.chars().count()));
    let rest = leads.iter().find_map(|lead| after.strip_prefix(lead).and_then(|r| r.strip_prefix(' ')))?.trim();
    // A payer known only by an address: "de (shop@example.org)".
    let inside = rest.strip_prefix(['(', '<']).and_then(|r| r.split([')', '>']).next());
    let name = inside.unwrap_or_else(|| rest.split(['(', '<', ',', '\n']).next().unwrap_or("")).trim().trim_end_matches('.').trim();
    (!name.is_empty() && name.chars().count() <= 60).then(|| name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What mail says about money, with the packs built in and one payment
    /// processor's wording added, as its users add it (`words::with_processor`).
    fn detect(card: &Card) -> Option<Payment> {
        super::detect(&crate::words::with_processor(), card)
    }

    fn card(from: &str, subject: &str, body: &str) -> Card {
        let raw = format!("From: {from}\r\nSubject: {subject}\r\nDate: Thu, 01 Oct 2026 10:00:00 +0200\r\nContent-Type: text/plain; charset=utf-8\r\n\r\n{body}\r\n");
        Card::from_bytes(raw.as_bytes()).unwrap()
    }

    const PAY: &str = "Pay Exemple <service@pay.example>";

    fn cents(p: &Payment) -> Option<i64> {
        p.amount.as_ref().map(|a| a.money.cents())
    }

    /// A processor's own wording is no shipped word: its authorisation is read once added.
    #[test]
    fn a_processors_wording_once_added() {
        let authorised = card("Pay Exemple <service@pay.example>", "SHOP: 23,98 € EUR", "Vous avez autorisé un paiement de 23,98 € EUR en faveur de SHOP");
        assert_eq!(super::detect(&crate::words::Words::builtin(), &authorised), None);
        assert_eq!(detect(&authorised).map(|p| (p.kind, p.party)), Some((PaymentKind::Paid, "SHOP".to_string())));
        // A receipt in plain words is read with the packs alone.
        let paid = card("Shop <shop@example.org>", "Votre paiement", "Vous avez payé 387,00 € à Exemple Retail.");
        assert_eq!(super::detect(&crate::words::Words::builtin(), &paid).map(|p| p.kind), Some(PaymentKind::Paid));
    }

    #[test]
    fn payments_received_and_made() {
        let received = detect(&card(
            "Pay Exemple <service@pay.example>",
            "Notification de paiement reçu",
            "Vous avez reçu un paiement de €12,00 EUR de Jean Exemple (jean@example.org).\r\nVous avez reçu ce message car…",
        ))
        .unwrap();
        assert_eq!((received.kind, cents(&received), received.party.as_str()), (PaymentKind::Received, Some(1200), "Jean Exemple"));
        let paid = detect(&card("Pay Exemple <service@pay.example>", "Reçu pour votre paiement à Exemple Retail", "Vous avez payé 387,00 € EUR à Exemple Retail.\r\n307,50 €")).unwrap();
        assert_eq!((paid.kind, cents(&paid), paid.party.as_str()), (PaymentKind::Paid, Some(38700), "Exemple Retail"));
        let authorised = detect(&card("Pay Exemple <service@pay.example>", "SHOP: 23,98 € EUR", "Vous avez autorisé un paiement de 23,98 € EUR en faveur de SHOP")).unwrap();
        assert_eq!((authorised.kind, cents(&authorised)), (PaymentKind::Paid, Some(2398)));
        let by_address = detect(&card("Pay Exemple <service@pay.example>", "Paiement reçu de shop@example.org", "Vous avez reçu un paiement de 500,00 € EUR de (shop@example.org).")).unwrap();
        assert_eq!(by_address.party, "shop@example.org");
    }

    #[test]
    fn orders_bills_and_settlements() {
        let order = detect(&card("Boutique <contact@shop.example>", "Commande FR1234 confirmée", "Sous-total\r\n76,50€\r\nTotal\r\n76,50€")).unwrap();
        assert_eq!((order.kind, cents(&order), order.party.as_str()), (PaymentKind::Order, Some(7650), "Boutique"));
        let bill = detect(&card("Opérateur <factures@telecom.example>", "Votre facture mobile est disponible", "Connectez-vous à votre espace.")).unwrap();
        assert_eq!((bill.kind, bill.amount), (PaymentKind::Bill, None));
        let settled = detect(&card("Opérateur <factures@telecom.example>", "Le règlement de votre facture a bien été effectué", "Merci.")).unwrap();
        assert_eq!(settled.kind, PaymentKind::Paid);
        let billed = detect(&card("Opérateur <factures@telecom.example>", "Votre facture est disponible", "Votre facture d'un montant de 29,99 € pour votre ligne est disponible.")).unwrap();
        assert_eq!((billed.kind, cents(&billed), billed.party.as_str()), (PaymentKind::Bill, Some(2999), "Opérateur"));
        let receipt = detect(&card(PAY, "Reçu de votre paiement", "Merci.\r\nPaiement de\r\nMoi Même\r\nPaiement à\r\nExemple Web\r\nshop@example.org\r\n84,99 € EUR\r\n-8,49 € EUR\r\n76,50 € EUR")).unwrap();
        assert_eq!((receipt.kind, cents(&receipt), receipt.party.as_str()), (PaymentKind::Paid, Some(7650), "Exemple Web"));
    }

    #[test]
    fn what_is_not_a_payment() {
        assert!(detect(&card("Pay Exemple <service@pay.example>", "Rapport d'activité prêt à être téléchargé", "Total 1 200,00 €")).is_none());
        assert!(detect(&card("Shop <news@shop.example>", "Votre commande a été expédiée", "Total 23,98 €")).is_none());
        assert!(detect(&card("Banque <notification@bank.example>", "Message de notification", "Un nouveau message vous attend.\r\nBanque Exemple, SA au capital de 1 000 000 €")).is_none());
        assert!(detect(&card("Ami <ami@example.org>", "Samedi ?", "On se voit samedi ?")).is_none());
        assert!(detect(&card("Opérateur <factures@telecom.example>", "Votre mandat SEPA a bien été signé", "Les prélèvements commenceront le mois prochain.")).is_none());
    }
}

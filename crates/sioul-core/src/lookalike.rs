// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Senders who borrow a name: "lnfos PrimeVlDEO <info@unrelated.example>".
//!
//! A signature proves the domain, not the name shown before it. Phishing
//! writes a brand or a public service in the display name, often with
//! lookalike letters (l for I, 0 for O, rn for m, Cyrillic а for a), and sends
//! from any domain. Names are compared by their skeleton, as Unicode's
//! confusable detection does (UTS #39 §4): both sides are reduced to one
//! prototype per family of lookalike letters. A brand named by a sender
//! outside its own domains is an impersonation. A brand whose name is also
//! an everyday word or a place (`BrandWords::everyday`: Orange, Apple, La
//! Poste…) is borrowed only when its name stands alone, or beside the words
//! of a service ("Apple Support", "Amazon.com"), so that "Orange County
//! Library" or "Apple Pie Bakery" borrow nothing; any other brand counts
//! anywhere in a name ("Crédit Agricole Nord de France", "PayPal Kundendienst").

/// A display name claiming a brand, or one of your own domains, that its address does not belong to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Impersonation {
    pub brand: String,
    pub domain: String,
}

/// The domains of your own addresses that can be impersonated: not those of
/// mail providers millions share (`shared`, `words::BrandWords::shared`),
/// where an address makes the domain nobody's own.
pub fn own_domains<'a>(shared: &[String], addresses: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let mut domains: Vec<String> = addresses
        .into_iter()
        .filter_map(|a| a.rsplit_once('@').map(|(_, d)| d.to_ascii_lowercase()))
        .filter(|d| !shared.iter().any(|s| s.trim().eq_ignore_ascii_case(d)))
        .collect();
    domains.sort();
    domains.dedup();
    domains
}

/// Whether a sender's display name claims a brand, or one of `own` (your domains),
/// from outside its domains. `brands`: the brands and public services, by the
/// words of their name, and their domains (a sender in one of them or below
/// it is genuine), and the words of a service, which a fake name wraps a brand
/// or a domain in ("PayPal Service", "janedoe.example Mail Admin";
/// `words::BrandWords`). A brand is claimed when its words are in the name
/// (`names`); an everyday one, when its name stands alone or beside such
/// words only (`borrows`).
pub fn impersonation(brands: &crate::words::BrandWords, name: Option<&str>, domain: Option<&str>, own: &[String]) -> Option<Impersonation> {
    let name = name?;
    let domain = domain?.to_ascii_lowercase();
    let raw = words(name);
    let words: Vec<String> = raw.iter().map(|w| skeleton(w)).collect();
    let service: Vec<String> = brands.service_words.iter().map(|w| skeleton(w)).collect();
    let everyday: Vec<String> = brands.everyday.iter().map(|b| crate::words::folded(b)).collect();
    let claims = |brand: &str, domains: &[String]| if everyday.contains(&crate::words::folded(brand)) { borrows(&raw, &words, brand, domains, &service) } else { names(&words, brand) };
    let inside = |domains: &[String]| domains.iter().map(|d| d.trim().to_ascii_lowercase()).any(|d| !d.is_empty() && (domain == d || domain.ends_with(&format!(".{d}"))));
    let brand = brands
        .brands
        .iter()
        .filter(|(brand, domains)| claims(brand, domains))
        .find(|(_, domains)| !inside(domains))
        .map(|(brand, _)| Impersonation { brand: brand.to_string(), domain: domain.clone() });
    brand.or_else(|| {
        own.iter()
            .find(|mine| !inside(std::slice::from_ref(mine)) && claims_own(&brands.service_words, &words, mine))
            .map(|mine| Impersonation { brand: mine.clone(), domain: domain.clone() })
    })
}

/// A name made of nothing but your domain and service words: "janedoe",
/// "Mail Admin janedoe.example". Your own name in full ("Jane Doe") is not
/// your domain run together, and a name with any other word is someone's.
fn claims_own(service_words: &[String], words: &[String], domain: &str) -> bool {
    let labels: Vec<String> = domain.split('.').map(skeleton).collect();
    let main = &labels[0];
    let generic: Vec<String> = service_words.iter().map(|w| skeleton(w)).collect();
    words.iter().any(|w| w == main) && words.iter().all(|w| labels.contains(w) || generic.contains(w))
}

/// Whether the brand's words appear in the name: one after the other, or run together.
fn names(words: &[String], brand: &str) -> bool {
    let wanted: Vec<String> = words_of_brand(brand);
    let joined = wanted.concat();
    let as_sequence = words.windows(wanted.len()).any(|w| w == wanted.as_slice());
    let run_together = words.iter().any(|w| *w == joined);
    as_sequence || run_together
}

/// Whether a name claims an everyday brand: its words are there, one after the other
/// or run together, and every other word of the name is a word of a service
/// (`service`, skeletons: "Support", "Service client", "Infos"), a number
/// ("Microsoft 365"), or a label of the brand's own domains ("Amazon.com",
/// "PayPal.fr"). The brand's name standing among other words ("Orange County
/// Library", "Apple Pie Bakery", "Café de la Poste") is someone else's name.
/// `raw`: the name's words as written; `words`: their skeletons.
fn borrows(raw: &[String], words: &[String], brand: &str, domains: &[String], service: &[String]) -> bool {
    let wanted: Vec<String> = words_of_brand(brand);
    if wanted.is_empty() {
        return false;
    }
    let joined = wanted.concat();
    // The words that are the brand's, wherever it is named.
    let mut theirs = vec![false; words.len()];
    let mut at = 0;
    while at < words.len() {
        if words[at..].starts_with(&wanted) {
            theirs[at..at + wanted.len()].iter_mut().for_each(|t| *t = true);
            at += wanted.len();
        } else {
            theirs[at] |= words[at] == joined;
            at += 1;
        }
    }
    if !theirs.contains(&true) {
        return false;
    }
    let labels: Vec<String> = domains.iter().flat_map(|d| d.split('.').map(skeleton).collect::<Vec<_>>()).filter(|l| !l.is_empty()).collect();
    words.iter().zip(raw).zip(&theirs).all(|((word, written), &brand)| brand || service.contains(word) || labels.contains(word) || written.chars().all(|c| c.is_ascii_digit()))
}

fn words_of_brand(brand: &str) -> Vec<String> {
    words(brand).iter().map(|w| skeleton(w)).collect()
}

/// Words of a name, split on everything that is not a letter or a digit.
fn words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(str::to_string).collect()
}

/// One prototype per family of lookalike letters, lowercase and without accents:
/// "PrimeVlDEO" and "Prime Video" share "prlmevldeo".
fn skeleton(word: &str) -> String {
    let lower: String = word.chars().map(crate::text::fold_char).flat_map(char::to_lowercase).map(prototype).collect();
    lower.replace("rn", "m").replace("vv", "w")
}

fn prototype(c: char) -> char {
    match c {
        'i' | 'l' | '1' | '|' | 'ı' | 'ӏ' | 'і' | 'ι' => 'l',
        '0' | 'о' | 'ο' => 'o',
        'а' | 'α' => 'a',
        'е' | 'ε' => 'e',
        'р' | 'ρ' => 'p',
        'с' | 'ϲ' => 'c',
        'у' => 'y',
        'х' | 'χ' => 'x',
        'ѕ' => 's',
        'ԁ' => 'd',
        'ν' => 'v',
        '5' => 's',
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// With the packs built in (French and English, France).
    fn impersonation(name: Option<&str>, domain: Option<&str>, own: &[String]) -> Option<Impersonation> {
        super::impersonation(&crate::words::Words::builtin().brands, name, domain, own)
    }

    fn own_domains<'a>(addresses: impl IntoIterator<Item = &'a str>) -> Vec<String> {
        super::own_domains(&crate::words::Words::builtin().brands.shared, addresses)
    }

    #[test]
    fn a_brand_of_yours_added() {
        let config: crate::config::Config = toml::from_str("[words]\nlanguages = [\"en\"]\ncountries = []\n[words.brands.brands]\nadd = { \"Banque Exemple\" = [\"banque-exemple.example\"] }\n").unwrap();
        let yours = crate::words::Words::of(&config);
        assert!(impersonation(Some("Banque Exemple"), Some("phish.example"), &[]).is_none());
        assert!(super::impersonation(&yours.brands, Some("Banque Exemple Sécurité"), Some("phish.example"), &[]).is_some());
        assert!(super::impersonation(&yours.brands, Some("Banque Exemple"), Some("mail.banque-exemple.example"), &[]).is_none());
        // Without France's names, its public bodies are no brands.
        assert!(super::impersonation(&yours.brands, Some("Assurance Maladie"), Some("remboursement.example"), &[]).is_none());
    }

    #[test]
    fn borrowed_names_are_caught() {
        let fake = impersonation(Some("lnfos PrimeVlDEO"), Some("unrelated.example"), &[]).unwrap();
        assert_eq!(fake.brand, "Prime Video");
        assert!(impersonation(Some("PayPaI Service"), Some("pay-secure.example"), &[]).is_some());
        assert!(impersonation(Some("Аmazon"), Some("deals.example"), &[]).is_some(), "Cyrillic А");
        assert!(impersonation(Some("Assurance Maladie"), Some("remboursement.example"), &[]).is_some());
    }

    #[test]
    fn your_own_domain_borrowed() {
        let own = own_domains(["jean@jeanexemple.example", "jean.exemple@gmail.com"]);
        assert_eq!(own, vec!["jeanexemple.example".to_string()]);
        let fake = impersonation(Some("jeanexemple"), Some("sales.example"), &own).unwrap();
        assert_eq!((fake.brand.as_str(), fake.domain.as_str()), ("jeanexemple.example", "sales.example"));
        assert!(impersonation(Some("Mail Admin jeanexemple.example"), Some("sales.example"), &own).is_some());
        // Your host's own mail, your full name, your own server: not impersonations.
        assert_eq!(impersonation(Some("Hosting Co - jeanexemple.example"), Some("host.example"), &own), None);
        assert_eq!(impersonation(Some("Jean Exemple"), Some("sales.example"), &own), None);
        assert_eq!(impersonation(Some("cPanel on jeanexemple.example"), Some("jeanexemple.example"), &own), None);
    }

    /// Every regional bank of Crédit Agricole mails from its own domain, and
    /// names the group: not a borrowed name (the France pack lists them all).
    #[test]
    fn a_regional_bank_names_its_group() {
        for domain in ["ca-norddefrance.fr", "mail.ca-languedoc.fr", "ca-reunion.fr", "ca-paris.fr", "credit-agricole.com"] {
            assert_eq!(impersonation(Some("Crédit Agricole"), Some(domain), &[]), None, "{domain}");
        }
        assert!(impersonation(Some("Credit Agricole"), Some("ca-securite.example"), &[]).is_some());
        assert!(impersonation(Some("Crédit Agricole Nord de France"), Some("ca-nordest.fr"), &[]).is_some(), "a domain that sends no mail is not one of theirs");
    }

    /// A brand whose name is an everyday word or a place counts alone, or
    /// beside the words of a service only; among other words, it is someone
    /// else's name. Any other brand counts anywhere in a name.
    #[test]
    fn an_everyday_brand_counts_alone_or_with_its_service() {
        // The true impersonations: an everyday brand alone, with service words, with its own domain's labels.
        for name in ["Apple", "Apple Support", "Amazon.com", "Amazon Customer Service", "Service client Orange", "Orange - Votre espace client", "UPS Delivery Notification", "Outlook Team", "Infos La Poste", "Service des Impôts des Particuliers"] {
            assert!(impersonation(Some(name), Some("phish.example"), &[]).is_some(), "{name}");
        }
        // Any other brand, anywhere in the name, as before.
        for name in ["Crédit Agricole Nord de France", "PayPal Kundendienst", "Microsoft 365", "Netflix Billing", "PayPal.fr Sécurité", "Mon conseiller BNP Paribas"] {
            assert!(impersonation(Some(name), Some("phish.example"), &[]).is_some(), "{name}");
        }
        // Everyday words and places, from their own domains.
        for name in ["Orange County Library", "Apple Pie Bakery", "Amazon Rainforest Trust", "Weekly Outlook", "Ups and Downs Club", "Café de la Poste", "Jean Orange", "Cabinet Martin, conseil en impôts", "Société Générale de Transports"] {
            assert_eq!(impersonation(Some(name), Some("their-own.example"), &[]), None, "{name}");
        }
        // Yours to mark: a brand of yours made everyday spares a name it stands in.
        let config: crate::config::Config = toml::from_str("[words]\nlanguages = [\"en\"]\ncountries = []\n[words.brands.brands]\nadd = { \"Banque Exemple\" = [\"banque-exemple.example\"] }\n[words.brands.everyday]\nadd = [\"Banque Exemple\"]\n").unwrap();
        let yours = crate::words::Words::of(&config);
        assert!(super::impersonation(&yours.brands, Some("Banque Exemple Bakery"), Some("bakery.example"), &[]).is_none());
        assert!(super::impersonation(&yours.brands, Some("Banque Exemple Support"), Some("phish.example"), &[]).is_some());
    }

    #[test]
    fn genuine_senders_and_other_names_pass() {
        assert_eq!(impersonation(Some("PayPal"), Some("news.paypal.com"), &[]), None);
        assert_eq!(impersonation(Some("Votre Assurance Maladie"), Some("app.ameli.fr"), &[]), None);
        assert_eq!(impersonation(Some("service@paypal.fr"), Some("paypal.fr"), &[]), None);
        assert_eq!(impersonation(Some("Médfrance"), Some("example.org"), &[]), None);
        assert_eq!(impersonation(Some("Freedom Mobile Club"), Some("example.org"), &[]), None);
        assert_eq!(impersonation(Some("Jean Exemple"), Some("example.org"), &[]), None);
        assert_eq!(impersonation(None, Some("example.org"), &[]), None);
    }
}

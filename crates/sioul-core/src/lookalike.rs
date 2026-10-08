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
//! outside its own domains is an impersonation.

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
/// it is genuine), and the words a fake "your provider" name wraps a domain in
/// ("janedoe.example Mail Admin"; `words::BrandWords`).
pub fn impersonation(brands: &crate::words::BrandWords, name: Option<&str>, domain: Option<&str>, own: &[String]) -> Option<Impersonation> {
    let name = name?;
    let domain = domain?.to_ascii_lowercase();
    let words: Vec<String> = words(name).iter().map(|w| skeleton(w)).collect();
    let inside = |domains: &[String]| domains.iter().map(|d| d.trim().to_ascii_lowercase()).any(|d| !d.is_empty() && (domain == d || domain.ends_with(&format!(".{d}"))));
    let brand = brands
        .brands
        .iter()
        .filter(|(brand, _)| names(&words, brand))
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

// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Countries as an invoice's data writes them (BT-40, BT-55, BT-80): ISO
//! 3166-1 alpha-2, the codes Factur-X's rules accept (FX-SCH-A-000568), and
//! XI, Northern Ireland's, which EN 16931's list adds. "UK" is GB, and "EL",
//! Greece's VAT prefix, is GR; a country written in words is found among the
//! names invoices use most, in English, in French and in its own language;
//! anything else is asked for before issue (`life::checks`), never taken for
//! the seller's country without a word.

/// ISO 3166-1 alpha-2, as the ISO keeps it (249 codes), and XI.
const CODES: [&str; 250] = [
    "AD", "AE", "AF", "AG", "AI", "AL", "AM", "AO", "AQ", "AR", "AS", "AT", "AU", "AW", "AX", "AZ", "BA", "BB", "BD", "BE", "BF", "BG", "BH", "BI", "BJ", "BL", "BM", "BN", "BO", "BQ", "BR", "BS", "BT", "BV", "BW", "BY", "BZ", "CA", "CC", "CD", "CF", "CG", "CH", "CI", "CK", "CL", "CM", "CN", "CO", "CR", "CU", "CV", "CW", "CX", "CY", "CZ", "DE", "DJ", "DK", "DM", "DO", "DZ", "EC", "EE", "EG", "EH", "ER", "ES", "ET", "FI", "FJ", "FK", "FM", "FO", "FR", "GA", "GB", "GD", "GE", "GF", "GG", "GH", "GI", "GL", "GM", "GN", "GP", "GQ", "GR", "GS", "GT", "GU", "GW", "GY", "HK", "HM", "HN", "HR", "HT", "HU", "ID", "IE", "IL", "IM", "IN", "IO", "IQ", "IR", "IS", "IT", "JE", "JM", "JO", "JP", "KE", "KG", "KH", "KI", "KM", "KN", "KP", "KR", "KW", "KY", "KZ", "LA", "LB", "LC", "LI", "LK", "LR", "LS", "LT", "LU", "LV", "LY", "MA", "MC", "MD", "ME", "MF", "MG", "MH", "MK", "ML", "MM", "MN", "MO", "MP", "MQ", "MR", "MS", "MT", "MU", "MV", "MW", "MX", "MY", "MZ", "NA", "NC", "NE", "NF", "NG", "NI", "NL", "NO", "NP", "NR", "NU", "NZ", "OM", "PA", "PE", "PF", "PG", "PH", "PK", "PL", "PM", "PN", "PR", "PS", "PT", "PW", "PY", "QA", "RE", "RO", "RS", "RU", "RW", "SA", "SB", "SC", "SD", "SE", "SG", "SH", "SI", "SJ", "SK", "SL", "SM", "SN", "SO", "SR", "SS", "ST", "SV", "SX", "SY", "SZ", "TC", "TD", "TF", "TG", "TH", "TJ", "TK", "TL", "TM", "TN", "TO", "TR", "TT", "TV", "TW", "TZ", "UA", "UG", "UM", "US", "UY", "UZ", "VA", "VC", "VE", "VG", "VI", "VN", "VU", "WF", "WS", "YE", "YT", "ZA", "ZM", "ZW", "XI",
];

/// Names written on invoices, lowercase, in English, French and the country's own language.
const NAMES: &[(&str, &str)] = &[
    ("france", "FR"),
    ("belgium", "BE"),
    ("belgique", "BE"),
    ("belgië", "BE"),
    ("belgie", "BE"),
    ("belgien", "BE"),
    ("germany", "DE"),
    ("allemagne", "DE"),
    ("deutschland", "DE"),
    ("switzerland", "CH"),
    ("suisse", "CH"),
    ("schweiz", "CH"),
    ("svizzera", "CH"),
    ("united kingdom", "GB"),
    ("royaume-uni", "GB"),
    ("royaume uni", "GB"),
    ("great britain", "GB"),
    ("grande-bretagne", "GB"),
    ("england", "GB"),
    ("angleterre", "GB"),
    ("scotland", "GB"),
    ("écosse", "GB"),
    ("wales", "GB"),
    ("pays de galles", "GB"),
    ("northern ireland", "GB"),
    ("luxembourg", "LU"),
    ("luxemburg", "LU"),
    ("netherlands", "NL"),
    ("the netherlands", "NL"),
    ("pays-bas", "NL"),
    ("pays bas", "NL"),
    ("nederland", "NL"),
    ("holland", "NL"),
    ("italy", "IT"),
    ("italie", "IT"),
    ("italia", "IT"),
    ("spain", "ES"),
    ("espagne", "ES"),
    ("españa", "ES"),
    ("espana", "ES"),
    ("portugal", "PT"),
    ("austria", "AT"),
    ("autriche", "AT"),
    ("österreich", "AT"),
    ("osterreich", "AT"),
    ("ireland", "IE"),
    ("irlande", "IE"),
    ("éire", "IE"),
    ("denmark", "DK"),
    ("danemark", "DK"),
    ("danmark", "DK"),
    ("sweden", "SE"),
    ("suède", "SE"),
    ("suede", "SE"),
    ("sverige", "SE"),
    ("finland", "FI"),
    ("finlande", "FI"),
    ("suomi", "FI"),
    ("norway", "NO"),
    ("norvège", "NO"),
    ("norvege", "NO"),
    ("norge", "NO"),
    ("iceland", "IS"),
    ("islande", "IS"),
    ("poland", "PL"),
    ("pologne", "PL"),
    ("polska", "PL"),
    ("czechia", "CZ"),
    ("czech republic", "CZ"),
    ("tchéquie", "CZ"),
    ("république tchèque", "CZ"),
    ("česko", "CZ"),
    ("slovakia", "SK"),
    ("slovaquie", "SK"),
    ("slovensko", "SK"),
    ("hungary", "HU"),
    ("hongrie", "HU"),
    ("magyarország", "HU"),
    ("slovenia", "SI"),
    ("slovénie", "SI"),
    ("slovenija", "SI"),
    ("croatia", "HR"),
    ("croatie", "HR"),
    ("hrvatska", "HR"),
    ("romania", "RO"),
    ("roumanie", "RO"),
    ("românia", "RO"),
    ("bulgaria", "BG"),
    ("bulgarie", "BG"),
    ("greece", "GR"),
    ("grèce", "GR"),
    ("grece", "GR"),
    ("cyprus", "CY"),
    ("chypre", "CY"),
    ("malta", "MT"),
    ("malte", "MT"),
    ("estonia", "EE"),
    ("estonie", "EE"),
    ("eesti", "EE"),
    ("latvia", "LV"),
    ("lettonie", "LV"),
    ("latvija", "LV"),
    ("lithuania", "LT"),
    ("lituanie", "LT"),
    ("lietuva", "LT"),
    ("liechtenstein", "LI"),
    ("monaco", "MC"),
    ("andorra", "AD"),
    ("andorre", "AD"),
    ("united states", "US"),
    ("united states of america", "US"),
    ("usa", "US"),
    ("états-unis", "US"),
    ("etats-unis", "US"),
    ("canada", "CA"),
    ("australia", "AU"),
    ("australie", "AU"),
    ("new zealand", "NZ"),
    ("nouvelle-zélande", "NZ"),
    ("japan", "JP"),
    ("japon", "JP"),
    ("china", "CN"),
    ("chine", "CN"),
    ("india", "IN"),
    ("inde", "IN"),
    ("brazil", "BR"),
    ("brésil", "BR"),
    ("mexico", "MX"),
    ("mexique", "MX"),
    ("morocco", "MA"),
    ("maroc", "MA"),
    ("tunisia", "TN"),
    ("tunisie", "TN"),
    ("algeria", "DZ"),
    ("algérie", "DZ"),
    ("senegal", "SN"),
    ("sénégal", "SN"),
    ("côte d’ivoire", "CI"),
    ("côte d'ivoire", "CI"),
    ("ivory coast", "CI"),
    ("israel", "IL"),
    ("israël", "IL"),
    ("turkey", "TR"),
    ("turquie", "TR"),
    ("türkiye", "TR"),
    ("ukraine", "UA"),
    ("south africa", "ZA"),
    ("afrique du sud", "ZA"),
    ("singapore", "SG"),
    ("singapour", "SG"),
    ("hong kong", "HK"),
    ("south korea", "KR"),
    ("corée du sud", "KR"),
    ("guadeloupe", "GP"),
    ("martinique", "MQ"),
    ("guyane", "GF"),
    ("french guiana", "GF"),
    ("la réunion", "RE"),
    ("réunion", "RE"),
    ("mayotte", "YT"),
    ("nouvelle-calédonie", "NC"),
    ("new caledonia", "NC"),
    ("polynésie française", "PF"),
    ("french polynesia", "PF"),
];

/// The ISO 3166-1 alpha-2 code `text` stands for: a code as written ("be"
/// too), "UK" as GB, "EL" as GR, a name of the table; none for anything else,
/// an empty text included.
pub fn code(text: &str) -> Option<&'static str> {
    let text = text.trim().trim_end_matches('.').trim();
    let upper = text.to_uppercase();
    match upper.as_str() {
        "UK" => return Some("GB"),
        "EL" => return Some("GR"),
        _ => {}
    }
    if let Some(code) = CODES.iter().find(|c| **c == upper) {
        return Some(code);
    }
    let lower = text.to_lowercase();
    NAMES.iter().find(|(name, _)| *name == lower).map(|(_, code)| *code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn countries_are_read_as_their_codes() {
        assert_eq!(code("FR"), Some("FR"));
        assert_eq!(code(" be "), Some("BE"));
        assert_eq!(code("UK"), Some("GB"));
        assert_eq!(code("EL"), Some("GR"));
        assert_eq!(code("Belgique"), Some("BE"));
        assert_eq!(code("DEUTSCHLAND"), Some("DE"));
        assert_eq!(code("États-Unis"), Some("US"));
        assert_eq!(code("XI"), Some("XI"));
        assert_eq!(code("ZZ"), None);
        assert_eq!(code("Atlantis"), None);
        assert_eq!(code(""), None);
        let mut sorted = CODES.to_vec();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), CODES.len(), "each code once");
        assert!(NAMES.iter().all(|(name, c)| CODES.contains(c) && name.to_lowercase() == *name), "names lowercase, codes known");
    }
}

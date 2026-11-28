// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The addresses of a client and of each part of its file
//! (docs/clients.md, "Addresses"): `sioul:client/<id>`, and below it
//! `/db/<kind>`, `/db/<kind>/<row>`, `/note/<note>`, `/description`,
//! `/file/<file>`. Every part is an id of Sioul's, never a name.

/// The prefix of every client's address.
pub const PREFIX: &str = "sioul:client/";

/// One address of a client or of a part of its file.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Address {
    Client { id: String },
    Kind { client: String, kind: String },
    Row { client: String, kind: String, row: String },
    Note { client: String, note: String },
    Description { client: String },
    File { client: String, file: String },
}

/// Whether a text is an id the clients' files take: letters, digits and
/// `-`, 1 to 64 of them. Every id is checked before it makes a path.
pub fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// A client's own address.
pub fn client_uri(id: &str) -> String {
    format!("{PREFIX}{id}")
}

impl Address {
    /// The address a text names; None for anything else, a hostile id included.
    pub fn parse(text: &str) -> Option<Address> {
        let rest = text.trim().strip_prefix(PREFIX)?;
        let parts: Vec<&str> = rest.split('/').collect();
        if !parts.iter().all(|p| valid_id(p)) && !matches!(parts.as_slice(), [_, "description"]) {
            return None;
        }
        let id = |s: &str| valid_id(s).then(|| s.to_string());
        Some(match parts.as_slice() {
            [client] => Address::Client { id: id(client)? },
            [client, "description"] => Address::Description { client: id(client)? },
            [client, "db", kind] => Address::Kind { client: id(client)?, kind: id(kind)? },
            [client, "db", kind, row] => Address::Row { client: id(client)?, kind: id(kind)?, row: id(row)? },
            [client, "note", note] => Address::Note { client: id(client)?, note: id(note)? },
            [client, "file", file] => Address::File { client: id(client)?, file: id(file)? },
            _ => return None,
        })
    }

    /// Its text.
    pub fn uri(&self) -> String {
        match self {
            Address::Client { id } => client_uri(id),
            Address::Kind { client, kind } => format!("{PREFIX}{client}/db/{kind}"),
            Address::Row { client, kind, row } => format!("{PREFIX}{client}/db/{kind}/{row}"),
            Address::Note { client, note } => format!("{PREFIX}{client}/note/{note}"),
            Address::Description { client } => format!("{PREFIX}{client}/description"),
            Address::File { client, file } => format!("{PREFIX}{client}/file/{file}"),
        }
    }

    /// The client's id, whatever part it names.
    pub fn client(&self) -> &str {
        match self {
            Address::Client { id } => id,
            Address::Kind { client, .. } | Address::Row { client, .. } | Address::Note { client, .. } | Address::Description { client } | Address::File { client, .. } => client,
        }
    }
}

/// A row's address.
pub fn row_uri(client: &str, kind: &str, row: &str) -> String {
    Address::Row { client: client.into(), kind: kind.into(), row: row.into() }.uri()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses_read_back() {
        let all = [
            Address::Client { id: "3f2a9c1e".into() },
            Address::Kind { client: "3f2a9c1e".into(), kind: "treatment".into() },
            Address::Row { client: "3f2a9c1e".into(), kind: "treatment".into(), row: "7c41".into() },
            Address::Note { client: "3f2a9c1e".into(), note: "a81d".into() },
            Address::Description { client: "3f2a9c1e".into() },
            Address::File { client: "3f2a9c1e".into(), file: "b902".into() },
        ];
        for a in &all {
            assert_eq!(Address::parse(&a.uri()).as_ref(), Some(a), "{}", a.uri());
            assert_eq!(a.client(), "3f2a9c1e");
        }
        assert_eq!(all[2].uri(), "sioul:client/3f2a9c1e/db/treatment/7c41");
        for hostile in ["sioul:client/", "sioul:client/a/../b", "sioul:client/a/db", "sioul:client/a/db/k/r/x", "sioul:client/a b", "sioul:client/a/note/", "sioul:contact/a", "sioul:client/a/file/x.y"] {
            assert_eq!(Address::parse(hostile), None, "{hostile}");
        }
    }
}

// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Extensions: programs apart from Sioul, one WebAssembly file each, that
//! bring data from a service Sioul does not speak to itself (docs/extensions.md,
//! the interface's reference). This crate holds what needs no network:
//!
//! - [`manifest`]: the signed list of what is offered, read and checked;
//! - [`signature`]: the manifest's detached OpenPGP signature, checked
//!   against the one key compiled in;
//! - [`reply`]: an extension's replies read, and an agenda made the files
//!   Sioul keeps, what it could not read kept from the last copy;
//! - [`state`]: what this device enabled, accepted and last saw.
//!
//! The engine that runs a file, what an extension may ask and the store live
//! in `sioul-sync` (`extensions`); the window's side in `sioul-app`.

pub mod manifest;
pub mod reply;
pub mod signature;
pub mod state;

/// The interface's version Sioul speaks (docs/extensions.md).
pub const INTERFACE: u32 = 1;

/// The lowest serial of a manifest this release of Sioul accepts, on any
/// device, its record lost or new (docs/extensions.md, "The signature"):
/// raised at each release of Sioul to the serial of the manifest published
/// then, so that an older signed manifest cannot be served again to a fresh
/// device. No expiry is asked of a signature (the publisher would have to
/// sign again on a timer): what this leaves open is said in the docs.
pub const LOWEST_SERIAL: u64 = 1;

/// The most an extension's secret holds: what every system's keyring keeps
/// (Windows' 2560 bytes, less a margin; `sioul_sync::secret::ENTRY_BYTES`).
pub const SECRET_BYTES: usize = 2400;

/// The kinds of data an extension may bring in this version.
pub const BRINGS: [&str; 1] = ["agenda"];

/// Whether `id` may name an extension: lower-case letters, digits and `-`,
/// 32 at most, neither beginning nor ending with `-`. It names a keyring
/// entry, a file and a log: nothing else may go in it.
pub fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 32 && !id.starts_with('-') && !id.ends_with('-') && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// The keyring entry of an extension's secret: named from its id, which the
/// manifest gives, never from what the extension says.
pub fn secret_name(id: &str) -> String {
    format!("extension {id}")
}

/// The folder of an agenda an extension brought, among the calendars kept on
/// this device (`vdir::LOCAL`): `ext-<extension>-<agenda>`, the same on every
/// device, so that the sharing's part "lists" carries it as one.
pub fn agenda_folder(extension: &str, agenda: &str) -> String {
    format!("ext-{extension}-{agenda}")
}

/// The extension and the agenda of a folder made by `agenda_folder`, given
/// the extensions known (an id may hold `-`: the longest that fits).
pub fn agenda_of_folder<'a>(folder: &'a str, known: &[&'a str]) -> Option<(&'a str, &'a str)> {
    let rest = folder.strip_prefix("ext-")?;
    known.iter().filter_map(|id| Some((*id, rest.strip_prefix(id)?.strip_prefix('-')?))).filter(|(_, agenda)| !agenda.is_empty()).max_by_key(|(id, _)| id.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_name_nothing_else() {
        for good in ["skolengo", "a", "school-1"] {
            assert!(valid_id(good), "{good}");
        }
        for bad in ["", "-a", "a-", "Skolengo", "a b", "a/b", "../x", "a.b", &"a".repeat(33)] {
            assert!(!valid_id(bad), "{bad}");
        }
        assert_eq!(secret_name("skolengo"), "extension skolengo");
        let folder = agenda_folder("my-ext", "pupil-1");
        assert_eq!(folder, "ext-my-ext-pupil-1");
        assert_eq!(agenda_of_folder(&folder, &["my", "my-ext"]), Some(("my-ext", "pupil-1")));
        assert_eq!(agenda_of_folder("ext-other-a", &["my-ext"]), None);
    }
}

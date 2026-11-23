// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Extensions: programs apart from Sioul, one WebAssembly file each, that
//! bring data from a service Sioul does not speak to itself (docs/extensions.md,
//! the interface's reference). This crate holds what needs no network:
//!
//! - [`manifest`]: the signed list of what is offered, read and checked;
//! - [`signature`]: the manifest's detached OpenPGP signature, checked
//!   against the one key compiled in;
//! - [`reply`]: an extension's replies read, an agenda written again from
//!   what was parsed, through an allow-list;
//! - [`door`]: an agenda written into a calendar of the person's own, through
//!   the agenda's door for events, its events marked as the extension's;
//! - [`cards`]: a set of contacts written into an address book of the
//!   person's own, through the contacts' door, each card bound to its person
//!   by a mark, additively, never deleted;
//! - [`state`]: what this device enabled, accepted and last saw.
//!
//! The engine that runs a file, what an extension may ask and the store live
//! in `sioul-sync` (`extensions`); the window's side in `sioul-app`.

pub mod cards;
pub mod door;
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

/// The kinds of data an extension may bring in this version: agendas, and
/// sets of contacts (added in version 1: an extension built before them
/// never hands any back).
pub const BRINGS: [&str; 2] = ["agenda", "contacts"];

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

/// The domain of the UIDs Sioul gives the events an extension brings (`event_uid`).
const UID_DOMAIN: &str = "extensions.sioul";

/// SHA-256 of `parts`, each preceded by its length (so that no two lists of
/// parts give the same bytes), in lower-case hexadecimal.
fn digest(parts: &[&str]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update((part.len() as u64).to_be_bytes());
        hasher.update(part.as_bytes());
    }
    hasher.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// An agenda of an extension, as its events' UIDs name it: 16 hexadecimal
/// characters, whatever the agenda's id holds.
pub fn agenda_key(extension: &str, agenda: &str) -> String {
    digest(&["agenda", extension, agenda])[..16].to_string()
}

/// The UID Sioul gives an event an extension brought, written into the
/// person's calendar: `<extension>.<agenda>.<lesson>@extensions.sioul`, the
/// agenda and the lesson as hashes of the extension's id, the agenda's and
/// the event's own UID, so that it is the same at every refresh and on every
/// device, and holds nothing those ids hold (a separator, a dot, a dash
/// first). The extension's id is the signed manifest's (`valid_id`). Its
/// form is the mark Sioul knows the extension's events by (`uid_mark`).
pub fn event_uid(extension: &str, agenda: &str, lesson: &str) -> String {
    format!("{extension}.{}.{}@{UID_DOMAIN}", agenda_key(extension, agenda), &digest(&["lesson", extension, agenda, lesson])[..32])
}

/// What the UID of an event Sioul wrote for an extension says: the
/// extension, and its agenda's key (`agenda_key`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UidMark {
    pub extension: String,
    pub agenda: String,
}

/// The mark of an event's UID when it has exactly the form `event_uid`
/// gives; none for any other (an event of the person's own, whatever its
/// title, time or categories).
pub fn uid_mark(uid: &str) -> Option<UidMark> {
    let rest = uid.strip_suffix(UID_DOMAIN)?.strip_suffix('@')?;
    let mut parts = rest.split('.');
    let (extension, agenda, lesson) = (parts.next()?, parts.next()?, parts.next()?);
    let hex = |text: &str, length: usize| text.len() == length && text.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
    (parts.next().is_none() && valid_id(extension) && hex(agenda, 16) && hex(lesson, 32)).then(|| UidMark { extension: extension.to_string(), agenda: agenda.to_string() })
}

/// The file name of an event Sioul writes for an extension, from its UID:
/// `<extension>-<lesson>.ics`, letters, digits and dashes, safe in a folder
/// and in a server's address, the same on every device.
pub fn event_file(uid: &str) -> String {
    let lesson = uid.split('@').next().and_then(|r| r.rsplit('.').next()).unwrap_or("");
    let extension = uid.split('.').next().unwrap_or("");
    format!("{extension}-{lesson}.ics")
}

/// A set of contacts of an extension, as its cards' marks name it: 16
/// hexadecimal characters, whatever the set's id holds, never one an agenda
/// of the same id gives (`agenda_key`).
pub fn set_key(extension: &str, set: &str) -> String {
    digest(&["contacts", extension, set])[..16].to_string()
}

/// The mark Sioul binds a card to a person of an extension's set with
/// (`cards`), made as an event's UID is (`event_uid`):
/// `<extension>.<set>.<person>@extensions.sioul`, the set and the person as
/// hashes of the extension's id, the set's and the person's own id, so that
/// it is the same at every refresh and on every device and holds nothing of
/// those ids (the person's id is the service's, never written as it is). It
/// is the UID of a card the door makes, and `uid_mark` reads it back.
pub fn person_mark(extension: &str, set: &str, person: &str) -> String {
    format!("{extension}.{}.{}@{UID_DOMAIN}", set_key(extension, set), &digest(&["person", extension, set, person])[..32])
}

/// The file name of a card Sioul makes for an extension, from its mark:
/// `<extension>-<person>.vcf`, letters, digits and dashes, the same on every device.
pub fn card_file(mark: &str) -> String {
    event_file(mark).replace(".ics", ".vcf")
}

/// The category an extension's events carry, and the name the event card
/// says they come from: its id with a capital ("skolengo" → "Skolengo"),
/// the same in every language and on every device, readable without its manifest.
pub fn category(extension: &str) -> String {
    let mut chars = extension.chars();
    chars.next().map(|first| first.to_uppercase().chain(chars).collect()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A person's mark: the same for the same ids, another for any other,
    /// never an agenda's of the same ids, read back by `uid_mark`, nothing
    /// of the set's or the person's id in it, its file safe.
    #[test]
    fn a_person_s_mark_is_stable_safe_and_read_back() {
        let mark = person_mark("bookings", "clients", "client-41");
        assert_eq!(mark, person_mark("bookings", "clients", "client-41"));
        assert_ne!(mark, person_mark("bookings", "clients", "client-42"));
        assert_ne!(mark, person_mark("bookings", "staff", "client-41"));
        assert_ne!(set_key("bookings", "clients"), agenda_key("bookings", "clients"), "a set and an agenda of one id are two");
        assert_ne!(mark, event_uid("bookings", "clients", "client-41"));
        assert_eq!(uid_mark(&mark), Some(UidMark { extension: "bookings".into(), agenda: set_key("bookings", "clients") }));
        assert!(!mark.contains("client") && mark.starts_with("bookings.") && mark.ends_with("@extensions.sioul"));
        for person in ["../../x", "a/b\\c", "x@y.z", "\r\nBEGIN:VCARD", ""] {
            let file = card_file(&person_mark("bookings", "clients", person));
            assert!(file.starts_with("bookings-") && file.ends_with(".vcf") && file.trim_end_matches(".vcf").bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'), "{file}");
        }
    }

    /// Version 1 takes agendas and contacts, nothing else.
    #[test]
    fn an_extension_may_bring_agendas_and_contacts() {
        let offer = |brings: &[&str]| manifest::Offer { brings: brings.iter().map(|b| b.to_string()).collect(), ..manifest::parse(manifest::SAMPLE).unwrap().offers.remove(0) };
        assert!(offer(&["agenda"]).usable());
        assert!(offer(&["contacts"]).usable());
        assert!(offer(&["agenda", "contacts"]).usable());
        assert!(!offer(&["contacts", "mail"]).usable(), "a kind this Sioul does not take");
        assert!(!offer(&[]).usable());
    }

    #[test]
    fn ids_name_nothing_else() {
        for good in ["skolengo", "a", "school-1"] {
            assert!(valid_id(good), "{good}");
        }
        for bad in ["", "-a", "a-", "Skolengo", "a b", "a/b", "../x", "a.b", &"a".repeat(33)] {
            assert!(!valid_id(bad), "{bad}");
        }
        assert_eq!(secret_name("skolengo"), "extension skolengo");
        assert_eq!(category("skolengo"), "Skolengo");
    }

    /// The UID of an extension's event: the same for the same ids, another
    /// for any other, its form read back, and nothing of the ids in it but
    /// the extension's own, whatever they hold (separators, dots, a dash first).
    #[test]
    fn an_event_s_uid_is_stable_safe_and_its_own_mark() {
        let uid = event_uid("skolengo", "pupil-1", "L-42@skolengo");
        assert_eq!(uid, event_uid("skolengo", "pupil-1", "L-42@skolengo"), "the same at every refresh");
        assert_eq!(uid_mark(&uid), Some(UidMark { extension: "skolengo".into(), agenda: agenda_key("skolengo", "pupil-1") }));
        assert!(uid.ends_with("@extensions.sioul") && uid.starts_with("skolengo."));
        // Ids that once made two names one (`a` + `b-c`, `a-b` + `c`) are two.
        assert_ne!(event_uid("a", "b-c", "x"), event_uid("a-b", "c", "x"));
        assert_ne!(agenda_key("a", "b-c"), agenda_key("a-b", "c"));
        assert_ne!(event_uid("e", "a", "b\u{0}c"), event_uid("e", "a\u{0}b", "c"), "parts never run into each other");
        for (agenda, lesson) in [("../../x", "../../../etc/passwd"), ("-dash", "a/b\\c"), ("a.b.c", "x@y.z"), ("", ""), ("é", "\r\nBEGIN:VEVENT")] {
            let uid = event_uid("skolengo", agenda, lesson);
            assert!(uid.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.' || b == b'@'), "{uid}");
            let file = event_file(&uid);
            assert!(file.ends_with(".ics") && file.trim_end_matches(".ics").bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-'), "{file}");
            assert!(uid_mark(&uid).is_some());
        }
        // Anything else is no mark: the person's own events, look-alikes.
        for other in ["L-42@skolengo", "skolengo.0123456789abcdef.0123456789abcdef0123456789abcdef@example.org", "Skolengo.0123456789abcdef.0123456789abcdef0123456789abcdef@extensions.sioul", "skolengo.0123456789ABCDEF.0123456789abcdef0123456789abcdef@extensions.sioul", "skolengo.0123456789abcdef.0123456789abcdef0123456789abcdef.x@extensions.sioul", "-x.0123456789abcdef.0123456789abcdef0123456789abcdef@extensions.sioul", ""] {
            assert_eq!(uid_mark(other), None, "{other}");
        }
    }
}

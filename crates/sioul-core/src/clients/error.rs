// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! What can go wrong with a client's file, in one plain sentence each. A
//! sentence never names a client nor quotes a file's content: a path in it
//! is made of ids alone (docs/clients.md, "Never in clear").

use jiff::civil::Date;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// A file that does not open from any copy, by its logical path: the caller keeps what it has.
    Damaged(String),
    /// Written elsewhere meanwhile: read again.
    Changed,
    /// A lock could not be taken.
    Locked,
    /// The disk is short: nothing was written.
    NoRoom,
    /// No store is open in this program yet.
    NotOpen,
    /// No such client, row, note, attachment, kind or field.
    NotFound,
    /// A row written with a newer template, or a field of a type this Sioul
    /// does not know: read only on this device.
    NewerTemplate,
    /// A row saved untouched: not kept.
    Empty,
    /// Erasing refused within the keeping times set, for a health professional's file.
    KeptUntil(Date),
    /// Its file is not on this device (kept on another).
    NotHere,
    /// A hostile id or path, a value of the wrong kind.
    Refused(&'static str),
    /// The system refused (permissions, a read-only disk).
    Io(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Damaged(path) => write!(f, "a file of the clients' files does not open ({path})"),
            Error::Changed => write!(f, "it was changed elsewhere meanwhile"),
            Error::Locked => write!(f, "the clients' files are busy"),
            Error::NoRoom => write!(f, "the disk is full: nothing was written"),
            Error::NotOpen => write!(f, "what Sioul keeps on this device is not open yet"),
            Error::NotFound => write!(f, "it is no longer there"),
            Error::NewerTemplate => write!(f, "written with a newer template: read only on this device"),
            Error::Empty => write!(f, "nothing was filled in"),
            Error::KeptUntil(day) => write!(f, "kept until {day} by the keeping times you set"),
            Error::NotHere => write!(f, "this client's file is not on this device"),
            Error::Refused(what) => write!(f, "refused: {what}"),
            Error::Io(what) => write!(f, "the system refused: {what}"),
        }
    }
}

impl std::error::Error for Error {}

impl Error {
    /// The client profiles' file (`sioul-clients.toml`) did not read or could
    /// not be written: one fixed sentence, since a parser's error quotes the
    /// line it stopped at (a legal name, an address) and the file's path
    /// (the second review, privacy).
    pub(crate) fn profiles(_why: String) -> Error {
        Error::Io("the client profiles' file did not read or could not be written".to_string())
    }

    /// A draft invoice could not be written: one fixed sentence, as `profiles`.
    pub(crate) fn draft(_why: String) -> Error {
        Error::Io("a draft invoice could not be written".to_string())
    }

    /// The id of its sentence for the person (`clients-error-…`, both languages;
    /// `clients-error-other` takes `$why`, this error's own words).
    pub fn words(&self) -> &'static str {
        match self {
            Error::Damaged(_) => "clients-error-damaged",
            Error::Changed => "clients-error-changed",
            Error::NotFound => "clients-error-not-found",
            Error::NewerTemplate => "clients-row-read-only",
            Error::Empty => "clients-error-empty",
            Error::KeptUntil(_) => "clients-erase-kept-until",
            Error::NotHere => "clients-error-not-here",
            Error::NotOpen => "clients-error-not-open",
            Error::Locked | Error::NoRoom | Error::Refused(_) | Error::Io(_) => "clients-error-other",
        }
    }
}

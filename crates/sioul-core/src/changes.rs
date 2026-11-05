// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! What changed, said by the one place that writes each kind of object
//! (docs/client.md, "Every view shows a change at once"): the persons'
//! store when it is written (`persons::change`), the agendas when what one
//! is changes (`agendas::set_field`), and so on for each kind. The window
//! listens (`listen`, once, as it starts) and tells every view that shows
//! that kind; a program without a window (the command line, the agents'
//! tools) has nobody listening, and nothing is said. What another program
//! or another device wrote reaches the window as the files it watches say
//! (the sharing's arrivals; agents' changes, `agentwrites`).

use std::cell::RefCell;
use std::sync::OnceLock;

/// A kind of object whose views read it again when it changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    /// The persons of `persons.toml` (docs/persons.md).
    Persons,
    /// What each agenda is (`[agenda.calendars]`: its role, its person, its
    /// area), and your availability's share (`[agenda.availability]`).
    Agendas,
    /// Money movements (docs/accounting.md, "Movements"): one made, changed
    /// or removed by hand, by an agent or from the command line
    /// (`movements::acts`), its fees, what a payout carried.
    Movements,
}

impl Kind {
    pub const ALL: [Kind; 3] = [Kind::Persons, Kind::Agendas, Kind::Movements];

    /// Its place among the kinds, for a set of them as bits.
    pub fn bit(self) -> u32 {
        1 << (self as u32)
    }
}

type Listener = Box<dyn Fn(Kind) + Send + Sync>;

static LISTENER: OnceLock<Listener> = OnceLock::new();

thread_local! {
    /// What this thread said, while a test asks (`heard`).
    static HEARD: RefCell<Option<Vec<Kind>>> = const { RefCell::new(None) };
}

/// The window's ear, set once as it starts; whether it was the first.
pub fn listen(listener: impl Fn(Kind) + Send + Sync + 'static) -> bool {
    LISTENER.set(Box::new(listener)).is_ok()
}

/// `kind` changed: said to the window, when one listens. Called by the one
/// place that writes that kind, after it wrote.
pub fn said(kind: Kind) {
    HEARD.with(|heard| {
        if let Some(heard) = heard.borrow_mut().as_mut() {
            heard.push(kind);
        }
    });
    if let Some(listener) = LISTENER.get() {
        listener(kind);
    }
}

/// What `run` said on this thread, in order (tests: a writer says its kind).
pub fn heard<R>(run: impl FnOnce() -> R) -> (R, Vec<Kind>) {
    let before = HEARD.with(|heard| heard.borrow_mut().replace(Vec::new()));
    let out = run();
    let said = HEARD.with(|heard| std::mem::replace(&mut *heard.borrow_mut(), before)).unwrap_or_default();
    (out, said)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A writer's word is heard on its own thread alone, nested asks each
    /// hearing their own; the kinds are distinct bits.
    #[test]
    fn what_is_said_is_heard_where_it_was_said() {
        let ((inner, ()), outer) = heard(|| {
            said(Kind::Persons);
            let inner = heard(|| said(Kind::Agendas));
            std::thread::spawn(|| said(Kind::Persons)).join().unwrap();
            (inner.1, ())
        });
        assert_eq!(outer, [Kind::Persons]);
        assert_eq!(inner, [Kind::Agendas]);
        let bits: Vec<u32> = Kind::ALL.iter().map(|k| k.bit()).collect();
        assert!(bits.iter().enumerate().all(|(i, b)| bits.iter().skip(i + 1).all(|c| b & c == 0)));
    }
}

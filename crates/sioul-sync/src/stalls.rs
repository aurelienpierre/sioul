// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The one place Sioul builds on ureq's transport, `ureq::unversioned`, which
//! ureq keeps out of its semver promise: any ureq release may change it.
//! Written for ureq 3.4 (3.4.2 in `Cargo.lock`, 9 October 2026). When an
//! update breaks the build here, this file alone changes; what it does stays
//! the same: each wait for the network bounded, not only each phase of a
//! request.
//!
//! ureq's own limits count a whole phase of a request (all of an answer's
//! body, for instance): an answer trickling in on a weak signal for longer
//! than that is cut although it moves, and one that stops is waited for until
//! the phase's end. Here each read and each write waits `stall` at most: an
//! answer goes on as long as it moves, and one that stops is given up soon.
//!
//! A shared agent (`shared_agent`) keeps its connections for requests that
//! ask different waits: each request says its own first, on its thread
//! (`ask`), since a connection kept in ureq's pool serves the next request
//! whichever opened it.

use std::cell::Cell;
use std::time::Duration;
use ureq::unversioned::resolver::DefaultResolver;
use ureq::unversioned::transport::{Buffers, ConnectionDetails, Connector, DefaultConnector, NextTimeout, Transport};

/// An agent with ureq's `config`, each wait for the network `stall` at most;
/// `patient`: the wait for an answer is not capped (a file sent whole, whose
/// bytes may wait in the system's buffers on a slow uplink while the answer
/// is awaited: bounded by the request's whole budget alone).
pub(crate) fn agent(config: ureq::config::Config, stall: Duration, patient: bool) -> ureq::Agent {
    ureq::Agent::with_parts(config, DefaultConnector::new().chain(Stalls { stall, patient, asked: false }), DefaultResolver::default())
}

/// An agent whose connections serve requests that ask different waits (the
/// sharing's one agent per process, `remote`): each wait as the request
/// running on this thread asked (`ask`); none asked yet on it, `stall`, not
/// patient.
pub(crate) fn shared_agent(config: ureq::config::Config, stall: Duration) -> ureq::Agent {
    ureq::Agent::with_parts(config, DefaultConnector::new().chain(Stalls { stall, patient: false, asked: true }), DefaultResolver::default())
}

thread_local! {
    /// What the last request run on this thread through a shared agent asked (`ask`).
    static ASKED: Cell<Option<(Duration, bool)>> = const { Cell::new(None) };
}

/// The waits of the next requests this thread runs through a shared agent
/// (`shared_agent`), their answers' bodies read included: each read and write
/// `stall` at most, the wait for an answer not capped when `patient`. Said
/// before each request; holds until the next is said. Other agents never read it.
pub(crate) fn ask(stall: Duration, patient: bool) {
    ASKED.set(Some((stall, patient)));
}

/// Each wait for the network, at most `stall`; `patient`: the waits for an
/// answer are not capped (`agent`'s); `asked`: both as the request running on
/// the thread asked (`shared_agent`).
#[derive(Debug)]
struct Stalls {
    stall: Duration,
    patient: bool,
    asked: bool,
}

impl Connector<Box<dyn Transport>> for Stalls {
    type Out = Stalling;

    fn connect(&self, _: &ConnectionDetails, chained: Option<Box<dyn Transport>>) -> Result<Option<Stalling>, ureq::Error> {
        Ok(chained.map(|transport| Stalling { transport, stall: self.stall, patient: self.patient, asked: self.asked }))
    }
}

#[derive(Debug)]
struct Stalling {
    transport: Box<dyn Transport>,
    stall: Duration,
    patient: bool,
    asked: bool,
}

impl Stalling {
    /// Its stall and whether it is patient, now: as asked on this thread, for a shared agent's.
    fn waits(&self) -> (Duration, bool) {
        if self.asked { ASKED.get().unwrap_or((self.stall, self.patient)) } else { (self.stall, self.patient) }
    }

    fn within(timeout: NextTimeout, stall: Duration) -> NextTimeout {
        NextTimeout { after: timeout.after.min(stall.into()), reason: timeout.reason }
    }
}

impl Transport for Stalling {
    fn buffers(&mut self) -> &mut dyn Buffers {
        self.transport.buffers()
    }

    fn transmit_output(&mut self, amount: usize, timeout: NextTimeout) -> Result<(), ureq::Error> {
        let timeout = Stalling::within(timeout, self.waits().0);
        self.transport.transmit_output(amount, timeout)
    }

    fn await_input(&mut self, timeout: NextTimeout) -> Result<bool, ureq::Error> {
        // An upload's answer, awaited while its last bytes drain from the
        // system's buffers on a slow uplink: within the whole budget alone.
        let (stall, patient) = self.waits();
        let timeout = if patient { timeout } else { Stalling::within(timeout, stall) };
        self.transport.await_input(timeout)
    }

    fn is_open(&mut self) -> bool {
        self.transport.is_open()
    }

    fn is_tls(&self) -> bool {
        self.transport.is_tls()
    }
}

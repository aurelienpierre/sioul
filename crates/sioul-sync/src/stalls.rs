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

use std::time::Duration;
use ureq::unversioned::resolver::DefaultResolver;
use ureq::unversioned::transport::{Buffers, ConnectionDetails, Connector, DefaultConnector, NextTimeout, Transport};

/// An agent with ureq's `config`, each wait for the network `stall` at most;
/// `patient`: the wait for an answer is not capped (a file sent whole, whose
/// bytes may wait in the system's buffers on a slow uplink while the answer
/// is awaited: bounded by the request's whole budget alone).
pub(crate) fn agent(config: ureq::config::Config, stall: Duration, patient: bool) -> ureq::Agent {
    ureq::Agent::with_parts(config, DefaultConnector::new().chain(Stalls(stall, patient)), DefaultResolver::default())
}

/// Each wait for the network, at most `.0`; `.1`: the waits for an answer
/// are not capped (`agent`'s `patient`).
#[derive(Debug)]
struct Stalls(Duration, bool);

impl Connector<Box<dyn Transport>> for Stalls {
    type Out = Stalling;

    fn connect(&self, _: &ConnectionDetails, chained: Option<Box<dyn Transport>>) -> Result<Option<Stalling>, ureq::Error> {
        Ok(chained.map(|transport| Stalling(transport, self.0, self.1)))
    }
}

#[derive(Debug)]
struct Stalling(Box<dyn Transport>, Duration, bool);

impl Stalling {
    fn within(&self, timeout: NextTimeout) -> NextTimeout {
        NextTimeout { after: timeout.after.min(self.1.into()), reason: timeout.reason }
    }
}

impl Transport for Stalling {
    fn buffers(&mut self) -> &mut dyn Buffers {
        self.0.buffers()
    }

    fn transmit_output(&mut self, amount: usize, timeout: NextTimeout) -> Result<(), ureq::Error> {
        let timeout = self.within(timeout);
        self.0.transmit_output(amount, timeout)
    }

    fn await_input(&mut self, timeout: NextTimeout) -> Result<bool, ureq::Error> {
        // An upload's answer, awaited while its last bytes drain from the
        // system's buffers on a slow uplink: within the whole budget alone.
        let timeout = if self.2 { timeout } else { self.within(timeout) };
        self.0.await_input(timeout)
    }

    fn is_open(&mut self) -> bool {
        self.0.is_open()
    }

    fn is_tls(&self) -> bool {
        self.0.is_tls()
    }
}

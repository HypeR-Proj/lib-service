// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Cooperative termination supervised by the process's userspace parent.
//!
//! No handler runs in this library, and no process authority is duplicated.
//! A supervisor reports a default action to its owner, which chooses whether
//! to terminate the child. Notification delivery is never proof of exit.

mod client;
mod protocol;
mod state;
mod supervisor;

pub use client::Client;
pub use supervisor::{Supervisor, prepare};

use hyper_os::handle::{ByteChannelObject, Rights};
use hyper_os::startup::StartupPurpose;

/// Optional, private parent-to-child control connection. It must not be shared
/// with other children or inherited as a child's own control connection.
pub const STARTUP: StartupPurpose<ByteChannelObject> = StartupPurpose::new(0x8004_0010);
pub const CHILD_RIGHTS: Rights = Rights::READ.union(Rights::WRITE).union(Rights::WAIT);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Disposition {
    Default = 0,
    Ignore = 1,
    Notify = 2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Signal {
    Terminate,
}

/// A delivered event retains its registration generation even if a subsequent
/// configuration acknowledgement has already been received.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Event {
    pub signal: Signal,
    pub generation: u64,
}

/// Result of handling a coalesced termination request, not process completion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Dispatch {
    Default,
    Ignore,
    Notified,
    Unavailable,
}

fn readiness(output_pending: bool) -> hyper_os::wait::ObjectSignals<ByteChannelObject> {
    use hyper_os::wait::ObjectSignals;
    let progress = if output_pending {
        ObjectSignals::<ByteChannelObject>::WRITABLE
    } else {
        ObjectSignals::<ByteChannelObject>::READABLE
    };
    progress.union(ObjectSignals::<ByteChannelObject>::PEER_CLOSED)
}

#[cfg(test)]
mod tests {
    #[test]
    fn blocked_output_waits_for_capacity_not_unread_input() {
        use hyper_os::handle::ByteChannelObject;
        use hyper_os::wait::ObjectSignals;
        let expected = ObjectSignals::<ByteChannelObject>::WRITABLE
            .union(ObjectSignals::<ByteChannelObject>::PEER_CLOSED);
        for bit in 0..64 {
            assert_eq!(
                super::readiness(true).is_present_in(1_u64 << bit),
                expected.is_present_in(1_u64 << bit)
            );
        }
    }
}

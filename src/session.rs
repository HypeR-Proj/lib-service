// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Startup contract for the virtual console manager.

use hyper_os::handle::{ByteChannelObject, Rights};
use hyper_os::startup::StartupPurpose;

use crate::StartupContract;

pub const STARTUP_DECISION_NAME: &str = "session.startup-decision";

/// Explicit one-shot selection by the current supervisor. Only bootstrap
/// selects Recovery; after ownership transfer formal init selects System.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShellMode {
    System,
    Recovery,
}

impl ShellMode {
    pub const fn encode(self) -> u8 {
        match self {
            Self::System => 1,
            Self::Recovery => 2,
        }
    }
    pub const fn decode(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::System),
            2 => Some(Self::Recovery),
            _ => None,
        }
    }
}

pub const STARTUP_DECISION: StartupPurpose<ByteChannelObject> = StartupPurpose::new(0x8002_0003);
pub const STARTUP_DECISION_CONTRACT: StartupContract = StartupContract::exact(
    STARTUP_DECISION_NAME,
    STARTUP_DECISION,
    Rights::WAIT.union(Rights::READ),
);

pub const CONSOLE_INPUT_NAME: &str = "session.console-input";
pub const CONSOLE_OUTPUT_NAME: &str = "session.console-output";

pub const CONSOLE_INPUT: StartupPurpose<ByteChannelObject> = StartupPurpose::new(0x8002_0001);
pub const CONSOLE_OUTPUT: StartupPurpose<ByteChannelObject> = StartupPurpose::new(0x8002_0002);

pub const CONSOLE_INPUT_CONTRACT: StartupContract = StartupContract::exact(
    CONSOLE_INPUT_NAME,
    CONSOLE_INPUT,
    Rights::WAIT.union(Rights::READ),
);
pub const CONSOLE_OUTPUT_CONTRACT: StartupContract = StartupContract::exact(
    CONSOLE_OUTPUT_NAME,
    CONSOLE_OUTPUT,
    Rights::WAIT.union(Rights::WRITE),
);
pub const STARTUP_CONTRACTS: &[StartupContract] = &[
    STARTUP_DECISION_CONTRACT,
    CONSOLE_INPUT_CONTRACT,
    CONSOLE_OUTPUT_CONTRACT,
    crate::vm::DELEGATED_CLIENT_CONNECTION_CONTRACT,
];

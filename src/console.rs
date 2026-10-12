// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Startup contract for the direction-attenuated physical Console workers.

use hyper_os::handle::{ByteChannelObject, ConsoleObject, Rights};
use hyper_os::startup::StartupPurpose;

use crate::StartupContract;

pub const SYSTEM_CONSOLE_NAME: &str = "console.system";
pub const DATA_CHANNEL_NAME: &str = "console.data";

pub const SYSTEM_CONSOLE: StartupPurpose<ConsoleObject> = StartupPurpose::new(0x8001_0001);
pub const DATA_CHANNEL: StartupPurpose<ByteChannelObject> = StartupPurpose::new(0x8001_0002);

pub const INPUT_SYSTEM_CONSOLE_CONTRACT: StartupContract = StartupContract::exact(
    SYSTEM_CONSOLE_NAME,
    SYSTEM_CONSOLE,
    Rights::WAIT.union(Rights::READ),
);
pub const OUTPUT_SYSTEM_CONSOLE_CONTRACT: StartupContract = StartupContract::exact(
    SYSTEM_CONSOLE_NAME,
    SYSTEM_CONSOLE,
    Rights::WAIT.union(Rights::WRITE),
);
pub const INPUT_DATA_CHANNEL_CONTRACT: StartupContract = StartupContract::exact(
    DATA_CHANNEL_NAME,
    DATA_CHANNEL,
    Rights::WAIT.union(Rights::WRITE),
);
pub const OUTPUT_DATA_CHANNEL_CONTRACT: StartupContract = StartupContract::exact(
    DATA_CHANNEL_NAME,
    DATA_CHANNEL,
    Rights::WAIT.union(Rights::READ),
);

pub const INPUT_STARTUP_CONTRACTS: &[StartupContract] =
    &[INPUT_SYSTEM_CONSOLE_CONTRACT, INPUT_DATA_CHANNEL_CONTRACT];
pub const OUTPUT_STARTUP_CONTRACTS: &[StartupContract] =
    &[OUTPUT_SYSTEM_CONSOLE_CONTRACT, OUTPUT_DATA_CHANNEL_CONTRACT];

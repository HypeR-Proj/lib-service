// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Formal session supervision. Bootstrap does not start this service.

use crate::StartupContract;
use hyper_os::handle::CapabilityChannelObject;
use hyper_os::startup::StartupPurpose;

pub const CONSOLE_CONTROLLER: StartupPurpose<CapabilityChannelObject> =
    StartupPurpose::new(0x8002_0001);
pub const LOG_CONNECTOR: StartupPurpose<CapabilityChannelObject> = StartupPurpose::new(0x8002_0002);
/// Optional one-shot startup gate. READY reports terminal availability; only
/// this release permits the first shell. Later session epochs do not use it.
pub const START: StartupPurpose<hyper_os::handle::ByteChannelObject> =
    StartupPurpose::new(0x8002_0003);
pub const START_MESSAGE: &[u8] = b"HYPER-SESSION-START/1";
pub const TASK_FACTORY_CONTRACT: StartupContract = StartupContract::exact(
    crate::process::TASK_FACTORY_NAME,
    hyper_os::startup::TASK_FACTORY,
    hyper_os::Rights::CREATE_PROCESS
        .union(hyper_os::Rights::RETAIN_PROCESS)
        .union(hyper_os::Rights::CREATE_TASK_GROUP)
        .union(hyper_os::Rights::DUPLICATE)
        .union(hyper_os::Rights::TRANSFER),
);
pub const STARTUP_CONTRACTS: &[StartupContract] = &[
    StartupContract::exact(
        "session.console-controller",
        CONSOLE_CONTROLLER,
        crate::console::CONNECTOR_RIGHTS,
    ),
    StartupContract::exact(
        "session.log-connector",
        LOG_CONNECTOR,
        crate::console::CONNECTOR_RIGHTS,
    ),
    TASK_FACTORY_CONTRACT,
    crate::vm::DELEGATED_CLIENT_CONNECTION_CONTRACT,
];

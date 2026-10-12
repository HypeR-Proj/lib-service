// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Conventional byte-channel purposes for Native command processes.

use hyper_os::handle::{ByteChannelObject, Rights};
use hyper_os::startup::StartupPurpose;

use crate::StartupContract;

pub const STANDARD_INPUT_NAME: &str = "stdio.input";
pub const STANDARD_OUTPUT_NAME: &str = "stdio.output";
pub const STANDARD_ERROR_NAME: &str = "stdio.error";

pub const STANDARD_INPUT: StartupPurpose<ByteChannelObject> = StartupPurpose::new(0x8003_0001);
pub const STANDARD_OUTPUT: StartupPurpose<ByteChannelObject> = StartupPurpose::new(0x8003_0002);
pub const STANDARD_ERROR: StartupPurpose<ByteChannelObject> = StartupPurpose::new(0x8003_0003);

pub const STANDARD_INPUT_CONTRACT: StartupContract = StartupContract::exact(
    STANDARD_INPUT_NAME,
    STANDARD_INPUT,
    Rights::WAIT
        .union(Rights::READ)
        .union(Rights::DUPLICATE)
        .union(Rights::TRANSFER),
)
.with_optional_rights(Rights::INSPECT);
pub const STANDARD_OUTPUT_CONTRACT: StartupContract = StartupContract::exact(
    STANDARD_OUTPUT_NAME,
    STANDARD_OUTPUT,
    Rights::WAIT
        .union(Rights::WRITE)
        .union(Rights::DUPLICATE)
        .union(Rights::TRANSFER),
);
pub const STANDARD_ERROR_CONTRACT: StartupContract = StartupContract::exact(
    STANDARD_ERROR_NAME,
    STANDARD_ERROR,
    Rights::WAIT
        .union(Rights::WRITE)
        .union(Rights::DUPLICATE)
        .union(Rights::TRANSFER),
);

pub const STARTUP_CONTRACTS: &[StartupContract] = &[
    STANDARD_INPUT_CONTRACT,
    STANDARD_OUTPUT_CONTRACT,
    STANDARD_ERROR_CONTRACT,
];

/// Ordinary services may keep stdio without authority to delegate it to children.
pub const APPLICATION_STARTUP_CONTRACTS: &[StartupContract] = &[
    StartupContract::exact(
        STANDARD_INPUT_NAME,
        STANDARD_INPUT,
        Rights::WAIT.union(Rights::READ),
    )
    .with_optional_rights(
        Rights::DUPLICATE
            .union(Rights::TRANSFER)
            .union(Rights::INSPECT),
    ),
    StartupContract::exact(
        STANDARD_OUTPUT_NAME,
        STANDARD_OUTPUT,
        Rights::WAIT.union(Rights::WRITE),
    )
    .with_optional_rights(Rights::DUPLICATE.union(Rights::TRANSFER)),
    StartupContract::exact(
        STANDARD_ERROR_NAME,
        STANDARD_ERROR,
        Rights::WAIT.union(Rights::WRITE),
    )
    .with_optional_rights(Rights::DUPLICATE.union(Rights::TRANSFER)),
];

/// A pipe or request-sized stream connector inherited by a child. Connections
/// stay private to each process; copying a connector never copies pending reads.
pub enum Inherited {
    Pipe(hyper_os::OwnedHandle<ByteChannelObject>),
    Stream(hyper_os::OwnedHandle<hyper_os::handle::CapabilityChannelObject>),
}

#[derive(Clone, Copy)]
pub enum StandardStream {
    Input,
    Output,
    Error,
}

impl Inherited {
    pub fn duplicate(&self, which: StandardStream) -> hyper_os::Result<Self> {
        Ok(match self {
            Self::Pipe(handle) => Self::Pipe(handle.duplicate(which.pipe_rights())?),
            Self::Stream(handle) => {
                Self::Stream(handle.duplicate(hyper_os::stream::CONNECTOR_RIGHTS)?)
            }
        })
    }
    pub fn add_to(
        self,
        builder: &hyper_os::task::ProcessBuilder,
        which: StandardStream,
    ) -> hyper_os::Result<()> {
        use hyper_os::handle::RightsOffer;
        match self {
            Self::Pipe(handle) => builder
                .add_handle_move(
                    handle,
                    which.pipe_purpose(),
                    RightsOffer::Exact(which.pipe_rights()),
                )
                .map_err(|failure| failure.error()),
            Self::Stream(handle) => builder
                .add_handle_move(
                    handle,
                    which.stream_purpose(),
                    RightsOffer::Exact(hyper_os::stream::CONNECTOR_RIGHTS),
                )
                .map_err(|failure| failure.error()),
        }
    }
}
impl StandardStream {
    fn pipe_purpose(self) -> u32 {
        match self {
            Self::Input => STANDARD_INPUT.as_raw(),
            Self::Output => STANDARD_OUTPUT.as_raw(),
            Self::Error => STANDARD_ERROR.as_raw(),
        }
    }
    fn stream_purpose(self) -> u32 {
        match self {
            Self::Input => hyper_os::stream::STANDARD_INPUT.as_raw(),
            Self::Output => hyper_os::stream::STANDARD_OUTPUT.as_raw(),
            Self::Error => hyper_os::stream::STANDARD_ERROR.as_raw(),
        }
    }
    fn pipe_rights(self) -> Rights {
        Rights::WAIT
            .union(Rights::DUPLICATE)
            .union(Rights::TRANSFER)
            .union(match self {
                Self::Input => Rights::READ,
                Self::Output | Self::Error => Rights::WRITE,
            })
    }
}

// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Initialized random bytes supplied by the trusted I/O appliance. No local
//! generator or weak fallback exists. Every call owns a private reply channel.

mod client;
mod codec;
pub use client::Client;
pub use codec::{MAX_BYTES, MAX_REPLY_BYTES, REQUEST_BYTES, Reply, Request, decode_reply};

use crate::StartupContract;
use hyper_os::handle::{CapabilityChannelObject, Rights};
use hyper_os::startup::StartupPurpose;

pub const CLIENT: StartupPurpose<CapabilityChannelObject> = StartupPurpose::new(0x800d_0001);
pub const SERVER: StartupPurpose<CapabilityChannelObject> = StartupPurpose::new(0x800d_0002);
pub const CLIENT_RIGHTS: Rights = Rights::WRITE.union(Rights::WAIT);
pub const DELEGATED_CLIENT_RIGHTS: Rights = CLIENT_RIGHTS
    .union(Rights::DUPLICATE)
    .union(Rights::TRANSFER);
pub const SERVER_RIGHTS: Rights = Rights::READ.union(Rights::WAIT);
pub const REPLY_RIGHTS: Rights = Rights::WRITE.union(Rights::WAIT);
pub const CLIENT_CONTRACT: StartupContract =
    StartupContract::exact("random.client", CLIENT, CLIENT_RIGHTS);
pub const DELEGATED_CLIENT_CONTRACT: StartupContract =
    StartupContract::exact("random.client", CLIENT, DELEGATED_CLIENT_RIGHTS);
pub const SERVER_CONTRACT: StartupContract =
    StartupContract::exact("random.server", SERVER, SERVER_RIGHTS);
pub const SERVER_STARTUP_CONTRACTS: &[StartupContract] = &[SERVER_CONTRACT];

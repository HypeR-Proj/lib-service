// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Bootstrap raw console ownership and independent service-log registration.
//!
//! These operations only bind byte transports. Line discipline belongs to the
//! formal terminal service, never the raw console worker.

use crate::StartupContract;
use hyper_os::capability_channel::{CapabilityChannel, CapabilityDisposition};
use hyper_os::handle::{
    ByteChannelObject, CapabilityChannelObject, ConsoleObject, HandleRef, OwnedHandle, Rights,
    RightsOffer,
};
use hyper_os::startup::StartupPurpose;
use hyper_os::wait::{ObjectSignals, WaitItem, wait_many};
use hyper_os::{Error, Result, Status};

pub const SYSTEM_CONSOLE: StartupPurpose<ConsoleObject> = StartupPurpose::new(0x8001_0001);
pub const CONTROL: StartupPurpose<CapabilityChannelObject> = StartupPurpose::new(0x8001_0003);
pub const LOG_CONNECTOR: StartupPurpose<CapabilityChannelObject> = StartupPurpose::new(0x8001_0004);
pub const SERVER_RIGHTS: Rights = Rights::READ.union(Rights::WAIT);
pub const CONNECTOR_RIGHTS: Rights = hyper_os::stream::CONNECTOR_RIGHTS;
pub const REPLY_RIGHTS: Rights = Rights::READ.union(Rights::WRITE).union(Rights::WAIT);
pub const CONTROL_MAGIC: [u8; 4] = *b"CONS";
pub const LOG_MAGIC: [u8; 4] = *b"LOGS";
pub const ATTACH: u32 = 1;
pub const RESET: u32 = 2;
pub const CONTROL_BYTES: usize = 16;
pub const MAX_LOG_NAME: usize = 64;
pub const STARTUP_CONTRACTS: &[StartupContract] = &[
    StartupContract::exact(
        "console.system",
        SYSTEM_CONSOLE,
        Rights::READ.union(Rights::WRITE).union(Rights::WAIT),
    ),
    StartupContract::exact("console.control", CONTROL, SERVER_RIGHTS),
    StartupContract::exact("console.logs", LOG_CONNECTOR, SERVER_RIGHTS),
];

/// A raw attachment survives only until reset or replacement. Clients inherit
/// connectors and establish private read/write sessions after process startup.
pub struct RawBinding {
    pub input: OwnedHandle<CapabilityChannelObject>,
    pub output: OwnedHandle<CapabilityChannelObject>,
}

fn header(operation: u32, epoch: u64) -> [u8; CONTROL_BYTES] {
    let mut bytes = [0; CONTROL_BYTES];
    bytes[..4].copy_from_slice(&CONTROL_MAGIC);
    bytes[4..8].copy_from_slice(&operation.to_le_bytes());
    bytes[8..].copy_from_slice(&epoch.to_le_bytes());
    bytes
}

fn send(
    channel: &CapabilityChannel,
    bytes: &[u8],
    offers: &mut [CapabilityDisposition<'_>],
) -> Result<()> {
    loop {
        match channel.try_send(bytes, offers) {
            Ok(()) => return Ok(()),
            Err(Error::Status(Status::WOULD_BLOCK)) => {
                wait_many(
                    &[WaitItem::new(
                        channel.as_handle_ref(),
                        ObjectSignals::<CapabilityChannelObject>::PEER_RECEIVING
                            .union(ObjectSignals::<CapabilityChannelObject>::PEER_CLOSED),
                    )],
                    hyper_os::DEADLINE_INFINITE,
                )?;
            }
            Err(error) => return Err(error),
        }
    }
}

fn acknowledgement(reply: &OwnedHandle<ByteChannelObject>) -> Result<()> {
    let mut bytes = [0; 8];
    if reply.as_byte_channel().receive(&mut bytes)? != bytes.len() {
        return Err(Error::InvalidResponse);
    }
    let status = Status::from_raw(i64::from_le_bytes(bytes));
    if status == Status::OK {
        Ok(())
    } else {
        Err(Error::Status(status))
    }
}

pub fn attach_raw(
    controller: HandleRef<'_, CapabilityChannelObject>,
    epoch: u64,
) -> Result<RawBinding> {
    // Replaced stream listeners retire asynchronously. Wait for their bounded
    // admission slots instead of allowing rapid restarts to grow worker count.
    for _ in 0..7 {
        match attach_once(controller, epoch) {
            Err(Error::Status(Status::BUSY)) => {
                let deadline =
                    hyper_os::time::deadline_after(core::time::Duration::from_millis(250))?;
                hyper_os::thread::sleep_until(deadline.as_raw())?;
            }
            result => return result,
        }
    }
    attach_once(controller, epoch)
}

fn attach_once(
    controller: HandleRef<'_, CapabilityChannelObject>,
    epoch: u64,
) -> Result<RawBinding> {
    let controller = CapabilityChannel::from_handle(controller.duplicate(CONNECTOR_RIGHTS)?);
    let (input_server, input) = CapabilityChannel::create()?;
    let (output_server, output) = CapabilityChannel::create()?;
    let (reply, remote) = hyper_os::channel::create_pair()?;
    let mut remote = Some(remote);
    let mut input_server = Some(input_server.into_handle());
    let mut output_server = Some(output_server.into_handle());
    send(
        &controller,
        &header(ATTACH, epoch),
        &mut [
            CapabilityDisposition::move_handle(&mut remote, RightsOffer::Exact(REPLY_RIGHTS))?,
            CapabilityDisposition::move_handle(
                &mut input_server,
                RightsOffer::Exact(SERVER_RIGHTS),
            )?,
            CapabilityDisposition::move_handle(
                &mut output_server,
                RightsOffer::Exact(SERVER_RIGHTS),
            )?,
        ],
    )?;
    acknowledgement(&reply)?;
    Ok(RawBinding {
        input: input.into_handle(),
        output: output.into_handle(),
    })
}

pub fn reset_raw(controller: HandleRef<'_, CapabilityChannelObject>, epoch: u64) -> Result<()> {
    let controller = CapabilityChannel::from_handle(controller.duplicate(CONNECTOR_RIGHTS)?);
    let (reply, remote) = hyper_os::channel::create_pair()?;
    let mut remote = Some(remote);
    send(
        &controller,
        &header(RESET, epoch),
        &mut [CapabilityDisposition::move_handle(
            &mut remote,
            RightsOffer::Exact(REPLY_RIGHTS),
        )?],
    )?;
    acknowledgement(&reply)
}

/// Gives the persistent raw worker a dedicated text-log reader. It renders LF
/// as CRLF while preserving existing CRLF; interactive streams remain binary.
/// A restarted service receives a new channel independent of other log sources.
pub fn register_log(
    connector: HandleRef<'_, CapabilityChannelObject>,
    name: &str,
    reader: OwnedHandle<ByteChannelObject>,
) -> Result<()> {
    if name.is_empty() || name.len() > MAX_LOG_NAME || name.as_bytes().contains(&0) {
        return Err(Error::InvalidResponse);
    }
    let connector = CapabilityChannel::from_handle(connector.duplicate(CONNECTOR_RIGHTS)?);
    let mut bytes = [0; 4 + MAX_LOG_NAME];
    bytes[..4].copy_from_slice(&LOG_MAGIC);
    bytes[4..4 + name.len()].copy_from_slice(name.as_bytes());
    let mut reader = Some(reader);
    send(
        &connector,
        &bytes[..4 + name.len()],
        &mut [CapabilityDisposition::move_handle(
            &mut reader,
            RightsOffer::Exact(SERVER_RIGHTS),
        )?],
    )
}

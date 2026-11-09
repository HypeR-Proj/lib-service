// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Userspace foreground membership. Records hold observation and request
//! capabilities only; the immediate parent retains the Process supervisor.
//! Membership is cooperative and is not a kernel `TaskGroup` security boundary.

use core::mem::MaybeUninit;
use hyper_os::capability_channel::{
    CapabilityChannel, CapabilityDisposition, CapabilityReceiveSlot,
};
use hyper_os::handle::{
    ByteChannelObject, CapabilityChannelObject, HandleRef, OwnedHandle, ProcessObject, Rights,
    RightsOffer,
};
use hyper_os::startup::{Startup, StartupPurpose};
use hyper_os::task::ProcessBuilder;
use hyper_os::wait::ObjectSignals;
use hyper_os::{Error, Result, Status};
extern crate alloc;
use alloc::vec::Vec;

pub const STARTUP: StartupPurpose<CapabilityChannelObject> = StartupPurpose::new(0x8004_0011);
pub const BROADCAST_STARTUP: StartupPurpose<ByteChannelObject> = StartupPurpose::new(0x8004_0012);
pub const CLIENT_RIGHTS: Rights = Rights::WRITE
    .union(Rights::WAIT)
    .union(Rights::TRANSFER)
    .union(Rights::DUPLICATE);
pub const BROADCAST_RIGHTS: Rights = Rights::WRITE
    .union(Rights::WAIT)
    .union(Rights::TRANSFER)
    .union(Rights::DUPLICATE);
const REQUEST_RIGHTS: Rights = Rights::WRITE.union(Rights::WAIT);
const MAX_MEMBERS: usize = 256;
const REGISTER: &[u8] = b"HJOBREG1";
const TERMINATE: &[u8] = b"HJOBTERM";

pub struct Client {
    channel: CapabilityChannel,
}

impl Client {
    pub fn create() -> Result<(CapabilityChannel, Self)> {
        let (listener, channel) = CapabilityChannel::create()?;
        Ok((listener, Self { channel }))
    }

    /// Wakes a blocked admission receiver with an invalid empty request.
    /// Only the owner's separate stop flag can authorize listener shutdown.
    pub fn wake_admission(&self) -> Result<()> {
        hyper_service_ipc::send(&self.channel, hyper_os::DEADLINE_INFINITE, b"", &mut [])
    }

    pub fn take(startup: &mut Startup<'_>) -> Result<Option<Self>> {
        Ok(startup.take_optional(STARTUP)?.map(|handle| Self {
            channel: CapabilityChannel::from_handle(handle),
        }))
    }

    pub fn add_to(&self, builder: &ProcessBuilder) -> Result<()> {
        builder.add_handle_duplicate(
            self.channel.as_handle_ref(),
            STARTUP.as_raw(),
            RightsOffer::Exact(CLIENT_RIGHTS),
        )
    }

    pub fn duplicate(&self) -> Result<Self> {
        Ok(Self {
            channel: CapabilityChannel::from_handle(
                self.channel.as_handle_ref().duplicate(CLIENT_RIGHTS)?,
            ),
        })
    }

    /// The child has started, but this parent must retain its supervisor and
    /// roll back the launch if registration fails. No management right moves.
    pub fn register(
        &self,
        observer: OwnedHandle<ProcessObject>,
        request: OwnedHandle<ByteChannelObject>,
    ) -> Result<()> {
        let until = hyper_service_ipc::deadline()?;
        let (reply, ack) = hyper_os::channel::create_pair()?;
        let mut observer = Some(observer);
        let mut request = Some(request);
        let mut ack = Some(ack);
        hyper_service_ipc::send(
            &self.channel,
            until,
            REGISTER,
            &mut [
                CapabilityDisposition::move_handle(
                    &mut observer,
                    RightsOffer::Exact(Rights::WAIT),
                )?,
                CapabilityDisposition::move_handle(
                    &mut request,
                    RightsOffer::Exact(REQUEST_RIGHTS),
                )?,
                CapabilityDisposition::move_handle(&mut ack, RightsOffer::Exact(Rights::WRITE))?,
            ],
        )?;
        let mut bytes = [0; 1];
        if reply.as_byte_channel().receive_until(&mut bytes, until)? != 1 {
            return Err(Error::InvalidResponse);
        }
        if bytes[0] != 0 {
            return Err(Error::Status(Status::RESOURCE_LIMIT));
        }
        Ok(())
    }
}

/// Explicit authority to request termination of the entire foreground job.
/// Registration authority alone never permits broadcasting to other members.
pub struct Broadcaster {
    channel: OwnedHandle<ByteChannelObject>,
    pending: bool,
}

impl Broadcaster {
    pub fn create() -> Result<(Requests, Self)> {
        let (requests, channel) = Requests::create()?;
        Ok((
            requests,
            Self {
                channel,
                pending: false,
            },
        ))
    }

    pub fn take(startup: &mut Startup<'_>) -> Result<Option<Self>> {
        Ok(startup
            .take_optional(BROADCAST_STARTUP)?
            .map(|channel| Self {
                channel,
                pending: false,
            }))
    }

    pub fn add_to(&self, builder: &ProcessBuilder) -> Result<()> {
        builder.add_handle_duplicate(
            self.channel.as_handle_ref(),
            BROADCAST_STARTUP.as_raw(),
            RightsOffer::Exact(BROADCAST_RIGHTS),
        )
    }

    pub fn duplicate(&self) -> Result<Self> {
        Ok(Self {
            channel: self.channel.duplicate(BROADCAST_RIGHTS)?,
            pending: false,
        })
    }

    /// Coalesces a local request. Call `poll` before sleeping afterwards.
    /// A full shared queue is not evidence that it contains a valid request.
    pub fn request_terminate(&mut self) {
        self.pending = true;
    }

    /// Attempts delivery without blocking. Only a successful write clears the
    /// local request; receiver-side consumption and member processing are later.
    pub fn poll(&mut self) -> Result<bool> {
        let channel = &self.channel;
        flush_broadcast(&mut self.pending, || {
            channel.as_byte_channel().try_send(TERMINATE)
        })
    }

    pub fn watch(
        &self,
    ) -> Option<(
        HandleRef<'_, ByteChannelObject>,
        ObjectSignals<ByteChannelObject>,
    )> {
        self.pending.then(|| {
            (
                self.channel.as_handle_ref(),
                ObjectSignals::<ByteChannelObject>::WRITABLE
                    .union(ObjectSignals::<ByteChannelObject>::PEER_CLOSED),
            )
        })
    }
}

fn flush_broadcast(pending: &mut bool, send: impl FnOnce() -> Result<()>) -> Result<bool> {
    if !*pending {
        return Ok(false);
    }
    match send() {
        Ok(()) => {
            *pending = false;
            Ok(true)
        }
        Err(Error::Status(Status::WOULD_BLOCK)) => Ok(false),
        Err(error) => Err(error),
    }
}

/// A private route to an immediate parent's event loop. Receiving a request
/// does not imply any default action: the parent consults its control record.
pub struct Requests {
    channel: OwnedHandle<ByteChannelObject>,
    closed: bool,
}

impl Requests {
    pub fn create() -> Result<(Self, OwnedHandle<ByteChannelObject>)> {
        let (channel, sender) = hyper_os::channel::create_pair()?;
        Ok((
            Self {
                channel,
                closed: false,
            },
            sender,
        ))
    }

    pub fn poll(&mut self) -> Result<bool> {
        if self.closed {
            return Ok(false);
        }
        let mut pending = false;
        for _ in 0..64 {
            let mut bytes = [0; 8];
            match self.channel.as_byte_channel().try_receive(&mut bytes) {
                Ok(8) if bytes == TERMINATE => pending = true,
                Ok(_) => {}
                Err(Error::MessageTooLarge { bytes, handles: 0 }) => {
                    // Malformed messages cannot poison a shared request queue.
                    // Allocate only on this rejection path, never per valid event.
                    let length = usize::try_from(bytes).map_err(|_| Error::InvalidResponse)?;
                    if length > hyper_os::channel::MAX_MESSAGE_BYTES {
                        return Err(Error::InvalidResponse);
                    }
                    let mut discard = Vec::new();
                    discard
                        .try_reserve_exact(length)
                        .map_err(|_| Error::Status(Status::NO_MEMORY))?;
                    discard.resize(length, 0);
                    self.channel.as_byte_channel().try_receive(&mut discard)?;
                }
                Err(Error::Status(Status::WOULD_BLOCK)) => break,
                Err(Error::Status(Status::PEER_CLOSED)) => {
                    self.closed = true;
                    break;
                }
                Err(error) => return Err(error),
            }
        }
        Ok(pending)
    }

    pub fn watch(
        &self,
    ) -> Option<(
        HandleRef<'_, ByteChannelObject>,
        ObjectSignals<ByteChannelObject>,
    )> {
        (!self.closed).then(|| {
            (
                self.channel.as_handle_ref(),
                ObjectSignals::<ByteChannelObject>::READABLE
                    .union(ObjectSignals::<ByteChannelObject>::PEER_CLOSED),
            )
        })
    }
}

struct Member {
    observer: OwnedHandle<ProcessObject>,
    request: OwnedHandle<ByteChannelObject>,
    pending: bool,
    unavailable: bool,
}

pub enum Arrival {
    Register {
        observer: OwnedHandle<ProcessObject>,
        request: OwnedHandle<ByteChannelObject>,
        ack: OwnedHandle<ByteChannelObject>,
    },
}

/// The owner drives admission and readiness; this table never spawns a thread.
pub struct Registry {
    members: Vec<Member>,
}

impl Registry {
    pub fn new() -> Self {
        Self {
            members: Vec::new(),
        }
    }

    pub fn admit(&mut self, arrival: Arrival) {
        match arrival {
            Arrival::Register {
                observer,
                request,
                ack,
            } => {
                let accepted = self.register(observer, request).is_ok();
                let _ = ack.as_byte_channel().try_send(&[u8::from(!accepted)]);
            }
        }
    }

    pub fn register(
        &mut self,
        observer: OwnedHandle<ProcessObject>,
        request: OwnedHandle<ByteChannelObject>,
    ) -> Result<()> {
        if self.members.len() >= MAX_MEMBERS {
            return Err(Error::Status(Status::RESOURCE_LIMIT));
        }
        self.members
            .try_reserve(1)
            .map_err(|_| Error::Status(Status::NO_MEMORY))?;
        self.members.push(Member {
            observer,
            request,
            pending: false,
            unavailable: false,
        });
        Ok(())
    }

    pub fn request_terminate(&mut self) {
        for member in &mut self.members {
            if !member.unavailable {
                member.pending = true;
            }
        }
    }

    pub fn poll(&mut self) -> Result<bool> {
        let mut progressed = false;
        let mut index = 0;
        while index < self.members.len() {
            let member = &mut self.members[index];
            match member.observer.as_process_supervisor().wait_terminated(0) {
                Ok(()) => {
                    self.members.swap_remove(index);
                    progressed = true;
                    continue;
                }
                Err(Error::Status(Status::TIMED_OUT)) => {}
                Err(error) => return Err(error),
            }
            if member.pending {
                match member.request.as_byte_channel().try_send(TERMINATE) {
                    Ok(()) => {
                        member.pending = false;
                        progressed = true;
                    }
                    Err(Error::Status(Status::WOULD_BLOCK)) => {}
                    Err(Error::Status(Status::PEER_CLOSED)) => {
                        member.pending = false;
                        member.unavailable = true;
                        progressed = true;
                    }
                    Err(error) => return Err(error),
                }
            }
            index += 1;
        }
        Ok(progressed)
    }

    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }

    pub fn member_count(&self) -> usize {
        self.members.len()
    }

    pub fn watches(
        &self,
    ) -> impl Iterator<
        Item = (
            HandleRef<'_, ByteChannelObject>,
            ObjectSignals<ByteChannelObject>,
        ),
    > {
        self.members.iter().filter(|m| m.pending).map(|m| {
            (
                m.request.as_handle_ref(),
                ObjectSignals::<ByteChannelObject>::WRITABLE
                    .union(ObjectSignals::<ByteChannelObject>::PEER_CLOSED),
            )
        })
    }

    pub fn observers(&self) -> impl Iterator<Item = HandleRef<'_, ProcessObject>> {
        self.members
            .iter()
            .map(|member| member.observer.as_handle_ref())
    }
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

/// Receives one bounded registration. The caller owns the blocking admission
/// context and must keep it separate from its policy and client reply loops.
pub fn accept(listener: &CapabilityChannel) -> Result<Arrival> {
    let mut bytes = [MaybeUninit::uninit(); 8];
    let mut slots = [
        CapabilityReceiveSlot::new::<ProcessObject>(Rights::WAIT),
        CapabilityReceiveSlot::new::<ByteChannelObject>(REQUEST_RIGHTS),
        CapabilityReceiveSlot::new::<ByteChannelObject>(Rights::WRITE),
    ];
    let message = listener.receive(hyper_os::DEADLINE_INFINITE, &mut bytes, &mut slots)?;
    if !registration_header(message.bytes(), message.capability_count()) {
        return Err(Error::InvalidResponse);
    }
    Ok(Arrival::Register {
        observer: slots[0].take()?.ok_or(Error::MissingHandle)?,
        request: slots[1].take()?.ok_or(Error::MissingHandle)?,
        ack: slots[2].take()?.ok_or(Error::MissingHandle)?,
    })
}

fn registration_header(bytes: &[u8], capabilities: usize) -> bool {
    bytes == REGISTER && capabilities == 3
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registration_authority_cannot_broadcast_or_accept_partial_transfers() {
        assert!(registration_header(REGISTER, 3));
        assert!(!registration_header(TERMINATE, 0));
        assert!(!registration_header(TERMINATE, 3));
        assert!(!registration_header(REGISTER, 2));
        assert!(!registration_header(REGISTER, 4));
        assert!(!registration_header(b"", 0));
        assert_ne!(STARTUP.as_raw(), BROADCAST_STARTUP.as_raw());
    }

    #[test]
    fn full_queue_does_not_acknowledge_an_unsent_broadcast() -> Result<()> {
        let mut pending = true;
        // Queue contents may be malformed frames from another sender. Draining
        // those frames cannot stand in for delivering this caller's request.
        for _ in 0..64 {
            assert!(!flush_broadcast(&mut pending, || Err(Error::Status(
                Status::WOULD_BLOCK
            )))?);
            assert!(pending);
        }
        assert!(flush_broadcast(&mut pending, || Ok(()))?);
        assert!(!pending);
        assert!(!flush_broadcast(&mut pending, || Err(
            Error::InvalidResponse
        ))?);
        Ok(())
    }

    #[test]
    fn failed_broadcast_retains_request_and_reports_failure() {
        let mut pending = true;
        assert_eq!(
            flush_broadcast(&mut pending, || Err(Error::Status(Status::PEER_CLOSED))),
            Err(Error::Status(Status::PEER_CLOSED))
        );
        assert!(pending);
    }
}

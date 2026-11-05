// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use super::Dispatch;
use super::protocol::{BYTES, Message};
use super::state::{Action, State};
use hyper_os::handle::{ByteChannelObject, HandleRef, OwnedHandle};
use hyper_os::wait::{ObjectSignals, WaitItem};
use hyper_os::{Error, Result, Status};

const POLL_BUDGET: usize = 8;

/// Prepares a private control connection before starting a child. The caller
/// moves the second endpoint into the child under [`super::STARTUP`]. Merely
/// creating this connection does not change the initial Default disposition.
pub fn prepare() -> Result<(Supervisor, OwnedHandle<ByteChannelObject>)> {
    let (parent, child) = hyper_os::channel::create_pair()?;
    Ok((
        Supervisor {
            channel: Some(parent),
            state: State::new(),
            output: None,
        },
        child,
    ))
}

/// Nonblocking parent-owned protocol driver. Process management authority stays
/// with the caller; this object only reports disposition outcomes.
pub struct Supervisor {
    channel: Option<OwnedHandle<ByteChannelObject>>,
    state: State,
    output: Option<Message>,
}

impl Supervisor {
    /// Adds one coalesced request. Call `poll` before sleeping afterwards.
    pub fn request_terminate(&mut self) {
        self.state.request();
    }

    pub fn as_handle_ref(&self) -> Option<HandleRef<'_, ByteChannelObject>> {
        self.channel.as_ref().map(OwnedHandle::as_handle_ref)
    }

    /// Writable readiness is requested only when an actual message is waiting,
    /// so ordinary idle connections do not spin on their free capacity.
    pub fn signals(&self) -> ObjectSignals<ByteChannelObject> {
        super::readiness(self.output.is_some())
    }

    pub fn wait_item(&self) -> Option<WaitItem<'_>> {
        self.as_handle_ref()
            .map(|handle| WaitItem::new(handle, self.signals()))
    }

    /// Services a bounded number of registrations/consumption acknowledgements,
    /// then handles the pending termination request. Never waits for the child.
    /// `now` is accepted for uniform supervisor reactors; this protocol does not
    /// impose a timeout or infer handler absence from elapsed time.
    pub fn poll(&mut self, _now: u64) -> Result<Option<Dispatch>> {
        let result = self.poll_inner();
        if result.is_err() {
            self.fail();
        }
        result
    }

    fn poll_inner(&mut self) -> Result<Option<Dispatch>> {
        if let Some(outcome) = self.flush()? {
            return Ok(Some(outcome));
        }
        if self.output.is_some() {
            return Ok(None);
        }
        for _ in 0..POLL_BUDGET {
            let Some(channel) = self.channel.as_ref() else {
                break;
            };
            let mut bytes = [0; BYTES];
            let count = match channel.as_byte_channel().try_receive(&mut bytes) {
                Ok(count) => count,
                Err(Error::Status(Status::WOULD_BLOCK)) => break,
                Err(Error::Status(Status::PEER_CLOSED)) => {
                    self.disconnect();
                    break;
                }
                Err(error) => return Err(error),
            };
            match Message::decode(&bytes[..count]).ok_or(Error::InvalidResponse)? {
                Message::Configure {
                    generation,
                    disposition,
                } => {
                    if !self.state.configure(generation, disposition) {
                        return Err(Error::InvalidResponse);
                    }
                    self.output = Some(Message::Configured { generation });
                }
                Message::Consumed { id } if self.state.consumed(id) => {}
                _ => return Err(Error::InvalidResponse),
            }
            if let Some(outcome) = self.flush()? {
                return Ok(Some(outcome));
            }
            if self.output.is_some() {
                return Ok(None);
            }
        }
        match self.state.action().ok_or(Error::InvalidResponse)? {
            Action::None => Ok(None),
            Action::Dispatch(outcome) => Ok(Some(outcome)),
            Action::Notify { generation, id } => {
                self.output = Some(Message::Terminate { generation, id });
                self.flush()
            }
        }
    }

    fn flush(&mut self) -> Result<Option<Dispatch>> {
        let Some(message) = self.output else {
            return Ok(None);
        };
        let Some(channel) = self.channel.as_ref() else {
            return Err(Error::InvalidResponse);
        };
        match channel.as_byte_channel().try_send(&message.encode()) {
            Ok(()) => {
                self.output = None;
                Ok(matches!(message, Message::Terminate { .. }).then_some(Dispatch::Notified))
            }
            Err(Error::Status(Status::WOULD_BLOCK)) => Ok(None),
            Err(Error::Status(Status::PEER_CLOSED)) => {
                let lost_notification = matches!(message, Message::Terminate { .. });
                self.disconnect();
                Ok(lost_notification.then_some(Dispatch::Unavailable))
            }
            Err(error) => Err(error),
        }
    }

    fn disconnect(&mut self) {
        self.channel = None;
        self.output = None;
        self.state.disconnected();
    }

    fn fail(&mut self) {
        self.channel = None;
        self.output = None;
        self.state.failed();
    }
}

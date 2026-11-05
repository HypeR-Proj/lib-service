// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use super::protocol::{BYTES, Message};
use super::{Disposition, Event, STARTUP};
use hyper_os::handle::{ByteChannelObject, HandleRef, OwnedHandle};
use hyper_os::startup::Startup;
use hyper_os::wait::{ObjectSignals, WaitItem};
use hyper_os::{Error, Result, Status};

/// A single-owner event receiver, usable in an application's ordinary reactor.
/// It installs no asynchronous callback and creates no hidden worker thread.
pub struct Client {
    channel: Option<OwnedHandle<ByteChannelObject>>,
    generation: u64,
    saved: Option<(Event, u64)>,
    acknowledge: Option<u64>,
    last_event: u64,
}

impl Client {
    pub fn take(startup: &mut Startup<'_>) -> Result<Option<Self>> {
        Ok(startup.take_optional(STARTUP)?.map(Self::from_handle))
    }

    pub fn from_handle(channel: OwnedHandle<ByteChannelObject>) -> Self {
        Self {
            channel: Some(channel),
            generation: 0,
            saved: None,
            acknowledge: None,
            last_event: 0,
        }
    }

    pub fn as_handle_ref(&self) -> Option<HandleRef<'_, ByteChannelObject>> {
        self.channel.as_ref().map(OwnedHandle::as_handle_ref)
    }

    pub fn signals(&self) -> ObjectSignals<ByteChannelObject> {
        super::readiness(self.acknowledge.is_some())
    }

    /// Drain `try_receive` before waiting: configure may have saved an event in
    /// userspace, for which the kernel channel no longer reports readability.
    pub fn wait_item(&self) -> Option<WaitItem<'_>> {
        self.as_handle_ref()
            .map(|handle| WaitItem::new(handle, self.signals()))
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Changes the disposition and waits for the parent's acknowledgement using
    /// one absolute deadline. The parent must be driving its supervisor reactor.
    /// Already-dispatched events survive the change with their original epoch;
    /// changing disposition does not retract delivery or run a callback.
    /// A transport/protocol failure closes this connection rather than allowing
    /// an ambiguous retry to consume a late acknowledgement as a new result.
    pub fn configure(&mut self, disposition: Disposition, until: u64) -> Result<()> {
        let result = self.configure_inner(disposition, until);
        if result.is_err() {
            self.channel = None;
        }
        result
    }

    fn configure_inner(&mut self, disposition: Disposition, until: u64) -> Result<()> {
        let generation = self
            .generation
            .checked_add(1)
            .ok_or(Error::InvalidResponse)?;
        if let Some(id) = self.acknowledge {
            self.channel()?
                .as_byte_channel()
                .send_until(&Message::Consumed { id }.encode(), until)?;
            self.acknowledge = None;
        }
        self.channel()?.as_byte_channel().send_until(
            &Message::Configure {
                generation,
                disposition,
            }
            .encode(),
            until,
        )?;
        loop {
            let mut bytes = [0; BYTES];
            let count = self
                .channel()?
                .as_byte_channel()
                .receive_until(&mut bytes, until)?;
            let message = Message::decode(&bytes[..count]).ok_or(Error::InvalidResponse)?;
            if message == (Message::Configured { generation }) {
                self.generation = generation;
                return Ok(());
            }
            let event = self.validate_event(message)?;
            if self.saved.replace(event).is_some() {
                return Err(Error::InvalidResponse);
            }
        }
    }

    /// Receives one event without blocking. A consumption acknowledgement only
    /// frees transport capacity; it does not claim cleanup or process exit.
    pub fn try_receive(&mut self) -> Result<Option<Event>> {
        let result = self.try_receive_inner();
        if result.is_err() {
            self.channel = None;
        }
        result
    }

    fn try_receive_inner(&mut self) -> Result<Option<Event>> {
        if !self.flush_acknowledgement()? {
            return Ok(None);
        }
        let (event, id) = if let Some(event) = self.saved.take() {
            event
        } else {
            let mut bytes = [0; BYTES];
            let count = match self.channel()?.as_byte_channel().try_receive(&mut bytes) {
                Ok(count) => count,
                Err(Error::Status(Status::WOULD_BLOCK)) => return Ok(None),
                Err(error) => return Err(error),
            };
            self.validate_event(Message::decode(&bytes[..count]).ok_or(Error::InvalidResponse)?)?
        };
        self.acknowledge = Some(id);
        // Keep the consumed event observable even if the parent disappeared
        // after sending it. Peer failure is reported on the next operation.
        if self.flush_acknowledgement().is_err() {
            self.channel = None;
        }
        Ok(Some(event))
    }

    pub fn receive_until(&mut self, until: u64) -> Result<Event> {
        loop {
            if until != hyper_os::DEADLINE_INFINITE
                && hyper_os::time::monotonic_now()?.as_nanoseconds() >= until
            {
                return Err(Error::Status(Status::TIMED_OUT));
            }
            if let Some(event) = self.try_receive()? {
                return Ok(event);
            }
            let item = self.wait_item().ok_or(Error::Status(Status::PEER_CLOSED))?;
            hyper_os::wait::wait_many(&[item], until)?;
        }
    }

    fn channel(&self) -> Result<&OwnedHandle<ByteChannelObject>> {
        self.channel
            .as_ref()
            .ok_or(Error::Status(Status::PEER_CLOSED))
    }

    fn validate_event(&mut self, message: Message) -> Result<(Event, u64)> {
        validate_event(message, self.generation, &mut self.last_event)
    }

    fn flush_acknowledgement(&mut self) -> Result<bool> {
        let Some(id) = self.acknowledge else {
            return Ok(true);
        };
        match self
            .channel()?
            .as_byte_channel()
            .try_send(&Message::Consumed { id }.encode())
        {
            Ok(()) => {
                self.acknowledge = None;
                Ok(true)
            }
            Err(Error::Status(Status::WOULD_BLOCK)) => Ok(false),
            Err(error) => Err(error),
        }
    }
}

fn validate_event(message: Message, generation: u64, last_event: &mut u64) -> Result<(Event, u64)> {
    let (event, id) = message.event().ok_or(Error::InvalidResponse)?;
    if event.generation > generation || last_event.checked_add(1) != Some(id) {
        return Err(Error::InvalidResponse);
    }
    *last_event = id;
    Ok((event, id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_old_registration_event_keeps_its_generation() -> Result<()> {
        let mut last = 0;
        let (event, id) = validate_event(
            Message::Terminate {
                generation: 1,
                id: 1,
            },
            2,
            &mut last,
        )?;
        assert_eq!(event.generation, 1);
        assert_eq!(id, 1);
        assert_eq!(last, 1);
        Ok(())
    }

    #[test]
    fn future_or_repeated_deliveries_are_rejected() -> Result<()> {
        let mut last = 0;
        assert_eq!(
            validate_event(
                Message::Terminate {
                    generation: 2,
                    id: 1
                },
                1,
                &mut last
            ),
            Err(Error::InvalidResponse)
        );
        validate_event(
            Message::Terminate {
                generation: 1,
                id: 1,
            },
            1,
            &mut last,
        )?;
        assert_eq!(
            validate_event(
                Message::Terminate {
                    generation: 1,
                    id: 1
                },
                1,
                &mut last
            ),
            Err(Error::InvalidResponse)
        );
        assert_eq!(
            validate_event(
                Message::Terminate {
                    generation: 1,
                    id: 3
                },
                1,
                &mut last
            ),
            Err(Error::InvalidResponse)
        );
        Ok(())
    }
}

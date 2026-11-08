// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use super::{MAX_BYTES, MAX_REPLY_BYTES, REPLY_RIGHTS, Request, decode_reply};
use hyper_os::capability_channel::{CapabilityChannel, CapabilityDisposition};
use hyper_os::handle::{CapabilityChannelObject, OwnedHandle, RightsOffer};
use hyper_os::{Error, Result, Status};

/// Owns a WRITE|WAIT connector. Concurrent calls use independent reply channels;
/// retaining or duplicating this capability is an explicit caller policy.
pub struct Client {
    channel: CapabilityChannel,
}
impl Client {
    #[must_use]
    pub fn from_handle(handle: OwnedHandle<CapabilityChannelObject>) -> Self {
        Self {
            channel: CapabilityChannel::from_handle(handle),
        }
    }
    /// Returns a completed prefix when a later chunk fails. Nonblocking means
    /// no entropy-readiness retry; admission and reply still honor `until`.
    /// Empty output succeeds without contacting the provider.
    pub fn read(&self, output: &mut [u8], nonblocking: bool, until: u64) -> Result<usize> {
        read_loop(
            output,
            nonblocking,
            until,
            |chunk, deadline| self.draw(chunk, deadline),
            || Ok(hyper_os::time::monotonic_now()?.as_nanoseconds()),
            hyper_os::thread::sleep_until,
        )
    }
    /// Completes the buffer using the original deadline. Failure may leave a
    /// successfully filled prefix; it never replaces missing bytes locally.
    pub fn fill(&self, output: &mut [u8], until: u64) -> Result<()> {
        let mut completed = 0;
        while completed < output.len() {
            let count = self.read(&mut output[completed..], false, until)?;
            if count == 0 {
                return Err(Error::InvalidResponse);
            }
            completed += count;
        }
        Ok(())
    }
    fn draw(&self, output: &mut [u8], until: u64) -> Result<()> {
        let request = Request::new(output.len(), until)?;
        let (receiver, sender) = hyper_os::channel::create_pair()?;
        let mut sender = Some(sender);
        hyper_service_ipc::send(
            &self.channel,
            until,
            &request.encode(),
            &mut [CapabilityDisposition::move_handle(
                &mut sender,
                RightsOffer::Exact(REPLY_RIGHTS),
            )?],
        )?;
        let mut frame = Secret([0; MAX_REPLY_BYTES]);
        let length = receiver
            .as_byte_channel()
            .receive_until(&mut frame.0, until)
            .map_err(|error| match error {
                Error::MessageTooLarge { .. } => Error::InvalidResponse,
                other => other,
            })?;
        output.copy_from_slice(decode_reply(&frame.0[..length], output.len())?);
        Ok(())
    }
}
struct Secret([u8; MAX_REPLY_BYTES]);
impl Drop for Secret {
    fn drop(&mut self) {
        super::codec::clear(&mut self.0);
    }
}

fn read_loop(
    output: &mut [u8],
    nonblocking: bool,
    until: u64,
    mut draw: impl FnMut(&mut [u8], u64) -> Result<()>,
    mut now: impl FnMut() -> Result<u64>,
    mut sleep: impl FnMut(u64) -> Result<()>,
) -> Result<usize> {
    let mut completed = 0;
    while completed < output.len() {
        let count = (output.len() - completed).min(MAX_BYTES);
        let attempt = (|| {
            loop {
                if now()? >= until {
                    return Err(Error::Status(Status::TIMED_OUT));
                }
                match draw(&mut output[completed..completed + count], until) {
                    Err(Error::Status(Status::WOULD_BLOCK)) if !nonblocking => {
                        let current = now()?;
                        if current >= until {
                            return Err(Error::Status(Status::TIMED_OUT));
                        }
                        let next = current
                            .saturating_add(10_000_000)
                            .min(u64::MAX - 1)
                            .min(until);
                        sleep(next)?;
                    }
                    result => return result,
                }
            }
        })();
        if let Err(error) = attempt {
            return if completed == 0 {
                Err(error)
            } else {
                Ok(completed)
            };
        }
        completed += count;
    }
    Ok(completed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::cell::Cell;

    #[test]
    fn original_deadline_caps_readiness_retries_and_sleep() {
        let clock = Cell::new(1);
        let attempts = Cell::new(0);
        let mut output = [7; 1];
        let result = read_loop(
            &mut output,
            false,
            15_000_000,
            |_, until| {
                assert_eq!(until, 15_000_000);
                attempts.set(attempts.get() + 1);
                Err(Error::Status(Status::WOULD_BLOCK))
            },
            || Ok(clock.get()),
            |until| {
                clock.set(until);
                Ok(())
            },
        );
        assert_eq!(result, Err(Error::Status(Status::TIMED_OUT)));
        assert_eq!(attempts.get(), 2);
        assert_eq!(clock.get(), 15_000_000);
        assert_eq!(output, [7]);
    }

    #[test]
    fn successful_prefix_survives_later_provider_failure() {
        let mut output = [0; MAX_BYTES + 1];
        let mut calls = 0;
        let result = read_loop(
            &mut output,
            false,
            99,
            |bytes, until| {
                assert_eq!(until, 99);
                calls += 1;
                if calls == 2 {
                    return Err(Error::Status(Status::IO_ERROR));
                }
                bytes.fill(3);
                Ok(())
            },
            || Ok(1),
            |_| Err(Error::InvalidResponse),
        );
        assert_eq!(result, Ok(MAX_BYTES));
        assert_eq!(&output[..MAX_BYTES], &[3; MAX_BYTES]);
        assert_eq!(output[MAX_BYTES], 0);
    }

    #[test]
    fn empty_nonblocking_expired_and_unsupported_never_invent_output() {
        assert_eq!(
            read_loop(
                &mut [],
                false,
                0,
                |_, _| Err(Error::InvalidResponse),
                || Err(Error::InvalidResponse),
                |_| Err(Error::InvalidResponse)
            ),
            Ok(0)
        );
        let mut output = [9; 1];
        assert_eq!(
            read_loop(
                &mut output,
                false,
                10,
                |_, _| Err(Error::InvalidResponse),
                || Ok(10),
                |_| Err(Error::InvalidResponse)
            ),
            Err(Error::Status(Status::TIMED_OUT))
        );
        for error in [
            Status::WOULD_BLOCK,
            Status::NOT_SUPPORTED,
            Status::ACCESS_DENIED,
            Status::PEER_CLOSED,
            Status::IO_ERROR,
        ] {
            assert_eq!(
                read_loop(
                    &mut output,
                    true,
                    10,
                    |_, _| Err(Error::Status(error)),
                    || Ok(1),
                    |_| Err(Error::InvalidResponse)
                ),
                Err(Error::Status(error))
            );
        }
        assert_eq!(output, [9]);
    }
}

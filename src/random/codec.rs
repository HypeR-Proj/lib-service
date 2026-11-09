// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use hyper_os::{Error, Result, Status};

pub const REQUEST_BYTES: usize = 32;
pub const MAX_BYTES: usize = 192;
pub const MAX_REPLY_BYTES: usize = 32 + MAX_BYTES;
const MAGIC: &[u8; 4] = b"HRNG";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Request {
    bytes: usize,
    deadline: u64,
}
impl Request {
    pub fn new(bytes: usize, deadline: u64) -> Result<Self> {
        if !(1..=MAX_BYTES).contains(&bytes) {
            return Err(Error::Status(Status::INVALID_ARGUMENT));
        }
        Ok(Self { bytes, deadline })
    }
    #[must_use]
    pub const fn bytes(self) -> usize {
        self.bytes
    }
    #[must_use]
    pub const fn deadline(self) -> u64 {
        self.deadline
    }
    #[must_use]
    pub fn encode(self) -> [u8; REQUEST_BYTES] {
        let mut frame = [0; REQUEST_BYTES];
        header(&mut frame, 1);
        frame[12..16].copy_from_slice(&(self.bytes as u32).to_le_bytes());
        frame[16..24].copy_from_slice(&self.deadline.to_le_bytes());
        frame
    }
    pub fn decode(frame: &[u8]) -> Result<Self> {
        if frame.len() != REQUEST_BYTES
            || !valid_header(frame, 1)
            || frame[8..12] != [0; 4]
            || frame[24..32] != [0; 8]
        {
            return Err(Error::InvalidResponse);
        }
        let bytes = u32::from_le_bytes(field(frame, 12)) as usize;
        Self::new(bytes, u64::from_le_bytes(field(frame, 16))).map_err(|_| Error::InvalidResponse)
    }
}

/// A bounded frame whose payload is erased on drop. It deliberately implements
/// neither Debug nor Clone; callers must not log its serialized bytes.
pub struct Reply {
    frame: [u8; MAX_REPLY_BYTES],
    length: usize,
}
impl Reply {
    pub fn success(bytes: &[u8]) -> Result<Self> {
        if !(1..=MAX_BYTES).contains(&bytes.len()) {
            return Err(Error::Status(Status::INVALID_ARGUMENT));
        }
        let mut result = Self::empty();
        result.frame[8..12].copy_from_slice(&(bytes.len() as u32).to_le_bytes());
        result.frame[32..32 + bytes.len()].copy_from_slice(bytes);
        result.length = 32 + bytes.len();
        Ok(result)
    }
    pub fn error(status: Status) -> Result<Self> {
        if !allowed_error(status) {
            return Err(Error::Status(Status::INVALID_ARGUMENT));
        }
        let mut result = Self::empty();
        result.frame[16..24].copy_from_slice(&status.as_raw().to_le_bytes());
        Ok(result)
    }
    fn empty() -> Self {
        let mut result = Self {
            frame: [0; MAX_REPLY_BYTES],
            length: 32,
        };
        header(&mut result.frame, 0x8001);
        result
    }
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.frame[..self.length]
    }
}
impl Drop for Reply {
    fn drop(&mut self) {
        clear(&mut self.frame);
    }
}

/// Validate the complete frame before exposing any payload to the caller.
/// Server failures preserve only the five explicitly declared status values.
pub fn decode_reply(frame: &[u8], expected: usize) -> Result<&[u8]> {
    if !(1..=MAX_BYTES).contains(&expected)
        || frame.len() < 32
        || frame.len() > MAX_REPLY_BYTES
        || !valid_header(frame, 0x8001)
        || frame[12..16] != [0; 4]
        || frame[24..32] != [0; 8]
    {
        return Err(Error::InvalidResponse);
    }
    let length = u32::from_le_bytes(field(frame, 8)) as usize;
    let status = Status::from_raw(i64::from_le_bytes(field(frame, 16)));
    if status != Status::OK {
        if frame.len() != 32 || length != 0 || !allowed_error(status) {
            return Err(Error::InvalidResponse);
        }
        return Err(Error::Status(status));
    }
    if length != expected || frame.len() != 32 + length {
        return Err(Error::InvalidResponse);
    }
    Ok(&frame[32..])
}
fn allowed_error(status: Status) -> bool {
    matches!(
        status,
        Status::INVALID_ARGUMENT
            | Status::NOT_SUPPORTED
            | Status::TIMED_OUT
            | Status::WOULD_BLOCK
            | Status::IO_ERROR
    )
}
fn header(frame: &mut [u8], operation: u16) {
    frame[..4].copy_from_slice(MAGIC);
    frame[4..6].copy_from_slice(&1u16.to_le_bytes());
    frame[6..8].copy_from_slice(&operation.to_le_bytes());
}
fn valid_header(frame: &[u8], operation: u16) -> bool {
    &frame[..4] == MAGIC
        && frame[4..6] == 1u16.to_le_bytes()
        && frame[6..8] == operation.to_le_bytes()
}
fn field<const N: usize>(frame: &[u8], offset: usize) -> [u8; N] {
    let mut bytes = [0; N];
    bytes.copy_from_slice(&frame[offset..offset + N]);
    bytes
}
pub(super) fn clear(bytes: &mut [u8]) {
    for byte in bytes {
        // SAFETY: each byte is an exclusive live reference into our own buffer.
        unsafe { core::ptr::write_volatile(byte, 0) };
    }
    core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

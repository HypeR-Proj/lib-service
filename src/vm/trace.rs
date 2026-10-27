// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Read-only VM ownership discovery. A separate admission message binds the
//! channel to this protocol; fleet lifecycle commands are never decoded there.

extern crate alloc;
#[cfg(test)]
use alloc::vec;
use alloc::{string::String, vec::Vec};

pub const CONNECT: &[u8] = b"HVMTRC01";
pub const REQUEST: &[u8] = b"HVMTRQ01";
const REPLY: &[u8] = b"HVMTRP01";
pub const MAX_THREADS: usize = 256;
pub const RECORD_BYTES: usize = 48;
pub const MAX_BYTES: usize = 8 + MAX_THREADS * RECORD_BYTES;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Thread {
    pub thread: u64,
    pub vcpu: u32,
    pub name: String,
}

pub fn encode(threads: &[Thread]) -> Option<Vec<u8>> {
    if threads.len() > MAX_THREADS {
        return None;
    }
    let mut bytes = Vec::with_capacity(8 + threads.len() * RECORD_BYTES);
    bytes.extend_from_slice(REPLY);
    for thread in threads {
        if !valid_name(&thread.name) {
            return None;
        }
        bytes.extend_from_slice(&thread.thread.to_le_bytes());
        bytes.extend_from_slice(&thread.vcpu.to_le_bytes());
        bytes.push(thread.name.len() as u8);
        bytes.extend_from_slice(&[0; 3]);
        bytes.extend_from_slice(thread.name.as_bytes());
        bytes.resize(bytes.len() + 32 - thread.name.len(), 0);
    }
    Some(bytes)
}

pub fn decode(bytes: &[u8]) -> Option<Vec<Thread>> {
    if bytes.len() < 8
        || bytes.len() > MAX_BYTES
        || &bytes[..8] != REPLY
        || !(bytes.len() - 8).is_multiple_of(RECORD_BYTES)
    {
        return None;
    }
    let mut threads = Vec::new();
    for record in bytes[8..].chunks_exact(RECORD_BYTES) {
        let n = usize::from(record[12]);
        if n > 32 || record[13..16] != [0; 3] || record[16 + n..].iter().any(|b| *b != 0) {
            return None;
        }
        let name = core::str::from_utf8(&record[16..16 + n]).ok()?;
        if !valid_name(name) {
            return None;
        }
        let thread = u64::from_le_bytes(record[..8].try_into().ok()?);
        if threads.iter().any(|t: &Thread| t.thread == thread) {
            return None;
        }
        threads.push(Thread {
            thread,
            vcpu: u32::from_le_bytes(record[8..12].try_into().ok()?),
            name: name.into(),
        });
    }
    Some(threads)
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 32
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ownership_preserves_full_ids_and_rejects_ambiguous_replies() -> Result<(), &'static str> {
        let threads = vec![Thread {
            thread: u64::MAX,
            vcpu: 0,
            name: "io".into(),
        }];
        let bytes = encode(&threads).ok_or("encode")?;
        assert_eq!(decode(&bytes), Some(threads.clone()));
        let mut invalid = bytes.clone();
        invalid[21] = 1;
        assert!(decode(&invalid).is_none());
        assert!(decode(&bytes[..bytes.len() - 1]).is_none());
        let duplicate = encode(&[threads[0].clone(), threads[0].clone()]).ok_or("encode")?;
        assert!(decode(&duplicate).is_none());
        assert!(decode(REQUEST).is_none());
        Ok(())
    }
}

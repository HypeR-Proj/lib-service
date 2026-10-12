// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! One binding generation distinguishes both frontends over a shared grant.

use super::{DISK, NETWORK};

pub const BOUND_MESSAGE: &[u8; 8] = b"HIOBND02";
pub const BOUND_BYTES: usize = 24;
pub const NETWORK_DEVICE_ID_BIT: u64 = 1 << 63;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Binding {
    pub generation: u64,
    pub devices: u32,
}

impl Binding {
    pub const fn valid(self) -> bool {
        self.generation != 0
            && self.generation < NETWORK_DEVICE_ID_BIT
            && self.devices != 0
            && self.devices & !(DISK | NETWORK) == 0
    }

    pub fn encode(self) -> Option<[u8; BOUND_BYTES]> {
        if !self.valid() {
            return None;
        }
        let mut bytes = [0; BOUND_BYTES];
        bytes[..8].copy_from_slice(BOUND_MESSAGE);
        bytes[8..16].copy_from_slice(&self.generation.to_le_bytes());
        bytes[16..20].copy_from_slice(&self.devices.to_le_bytes());
        Some(bytes)
    }

    pub fn decode(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != BOUND_BYTES || &bytes[..8] != BOUND_MESSAGE {
            return None;
        }
        let binding = Self {
            generation: u64::from_le_bytes(bytes[8..16].try_into().ok()?),
            devices: u32::from_le_bytes(bytes[16..20].try_into().ok()?),
        };
        (binding.encode()?.as_slice() == bytes).then_some(binding)
    }

    pub fn device_id(self, device: u32) -> Option<u64> {
        if !self.valid() || self.devices & device == 0 {
            return None;
        }
        match device {
            DISK => Some(self.generation),
            NETWORK => Some(self.generation | NETWORK_DEVICE_ID_BIT),
            _ => None,
        }
    }
}

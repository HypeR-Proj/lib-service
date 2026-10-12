// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Canonical broker admission for all devices sharing one guest-memory grant.

pub const CONNECT_BYTES: usize = 96;
pub const DISK: u32 = 1;
pub const NETWORK: u32 = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Connection<'a> {
    pub client: u32,
    pub volume: Option<&'a str>,
    pub network: Option<&'a str>,
    pub mac: [u8; 6],
}

impl Connection<'_> {
    pub const fn devices(self) -> u32 {
        (if self.volume.is_some() { DISK } else { 0 })
            | (if self.network.is_some() { NETWORK } else { 0 })
    }

    pub fn encode(self) -> Option<[u8; CONNECT_BYTES]> {
        if !(1..=127).contains(&self.client)
            || self.devices() == 0
            || self.volume.is_some_and(|name| !valid_volume(name))
            || self.network.is_some_and(|name| !valid_name(name))
            || if self.network.is_some() {
                self.mac[0] & 3 != 2
            } else {
                self.mac != [0; 6]
            }
        {
            return None;
        }
        let mut bytes = [0; CONNECT_BYTES];
        bytes[..8].copy_from_slice(b"HIOCONN2");
        bytes[8..12].copy_from_slice(&self.client.to_le_bytes());
        bytes[12..16].copy_from_slice(&self.devices().to_le_bytes());
        if let Some(volume) = self.volume {
            bytes[16..16 + volume.len()].copy_from_slice(volume.as_bytes());
        }
        if let Some(network) = self.network {
            bytes[48..48 + network.len()].copy_from_slice(network.as_bytes());
        }
        bytes[80..86].copy_from_slice(&self.mac);
        Some(bytes)
    }
}

pub fn encode_connect(client: u32, volume: &str) -> Option<[u8; CONNECT_BYTES]> {
    Connection {
        client,
        volume: Some(volume),
        network: None,
        mac: [0; 6],
    }
    .encode()
}

pub fn decode_connect(bytes: &[u8]) -> Option<Connection<'_>> {
    if bytes.len() != CONNECT_BYTES || &bytes[..8] != b"HIOCONN2" {
        return None;
    }
    let devices = u32::from_le_bytes(bytes[12..16].try_into().ok()?);
    let connection = Connection {
        client: u32::from_le_bytes(bytes[8..12].try_into().ok()?),
        volume: if devices & DISK != 0 {
            Some(wire_name(&bytes[16..48])?)
        } else {
            None
        },
        network: if devices & NETWORK != 0 {
            Some(wire_name(&bytes[48..80])?)
        } else {
            None
        },
        mac: bytes[80..86].try_into().ok()?,
    };
    // Re-encoding rejects unknown flags, hidden fields and every nonzero pad.
    (connection.encode()?.as_slice() == bytes).then_some(connection)
}

fn wire_name(bytes: &[u8]) -> Option<&str> {
    let length = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    core::str::from_utf8(&bytes[..length]).ok()
}

/// A deployment token shared by board policy and the Linux bootstrap format.
pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 31
        && name.as_bytes()[0].is_ascii_lowercase()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

/// Existing disk identities permit a full 32-byte token without a terminator.
pub fn valid_volume(volume: &str) -> bool {
    !volume.is_empty()
        && volume.len() <= 32
        && volume
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_".contains(&byte))
}

/// Accept only canonical, locally administered unicast Ethernet addresses.
pub fn parse_mac(value: &str) -> Option<[u8; 6]> {
    fn hex(byte: u8) -> Option<u8> {
        match byte {
            b'0'..=b'9' => Some(byte - b'0'),
            b'a'..=b'f' => Some(byte - b'a' + 10),
            _ => None,
        }
    }
    let bytes = value.as_bytes();
    if bytes.len() != 17 {
        return None;
    }
    let mut mac = [0; 6];
    for (index, octet) in mac.iter_mut().enumerate() {
        let offset = index * 3;
        *octet = hex(bytes[offset])? << 4 | hex(bytes[offset + 1])?;
        if index != 5 && bytes[offset + 2] != b':' {
            return None;
        }
    }
    (mac[0] & 3 == 2).then_some(mac)
}

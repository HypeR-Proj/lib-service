// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Fixed-size, canonical records with no unbounded or hidden fields.

use super::{Disposition, Event, Signal};

pub(super) const BYTES: usize = 32;
const MAGIC: &[u8; 8] = b"HSIGCTL1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Message {
    Configure {
        generation: u64,
        disposition: Disposition,
    },
    Configured {
        generation: u64,
    },
    Terminate {
        generation: u64,
        id: u64,
    },
    Consumed {
        id: u64,
    },
}

impl Message {
    pub(super) fn encode(self) -> [u8; BYTES] {
        let (kind, value, generation, id) = match self {
            Self::Configure {
                generation,
                disposition,
            } => (1u32, disposition as u32, generation, 0),
            Self::Configured { generation } => (2, 0, generation, 0),
            Self::Terminate { generation, id } => (3, 0, generation, id),
            Self::Consumed { id } => (4, 0, 0, id),
        };
        let mut bytes = [0; BYTES];
        bytes[..8].copy_from_slice(MAGIC);
        bytes[8..12].copy_from_slice(&kind.to_le_bytes());
        bytes[12..16].copy_from_slice(&value.to_le_bytes());
        bytes[16..24].copy_from_slice(&generation.to_le_bytes());
        bytes[24..32].copy_from_slice(&id.to_le_bytes());
        bytes
    }

    pub(super) fn decode(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != BYTES || &bytes[..8] != MAGIC {
            return None;
        }
        let kind = u32::from_le_bytes(bytes[8..12].try_into().ok()?);
        let value = u32::from_le_bytes(bytes[12..16].try_into().ok()?);
        let generation = u64::from_le_bytes(bytes[16..24].try_into().ok()?);
        let id = u64::from_le_bytes(bytes[24..32].try_into().ok()?);
        let message = match kind {
            1 if generation != 0 && id == 0 => Self::Configure {
                generation,
                disposition: match value {
                    0 => Disposition::Default,
                    1 => Disposition::Ignore,
                    2 => Disposition::Notify,
                    _ => return None,
                },
            },
            2 if value == 0 && generation != 0 && id == 0 => Self::Configured { generation },
            3 if value == 0 && generation != 0 && id != 0 => Self::Terminate { generation, id },
            4 if value == 0 && generation == 0 && id != 0 => Self::Consumed { id },
            _ => return None,
        };
        Some(message)
    }

    pub(super) fn event(self) -> Option<(Event, u64)> {
        match self {
            Self::Terminate { generation, id } => Some((
                Event {
                    signal: Signal::Terminate,
                    generation,
                },
                id,
            )),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_are_exact_length_and_reject_unknown_fields() {
        let messages = [
            Message::Configure {
                generation: 1,
                disposition: Disposition::Default,
            },
            Message::Configure {
                generation: 2,
                disposition: Disposition::Ignore,
            },
            Message::Configure {
                generation: 3,
                disposition: Disposition::Notify,
            },
            Message::Configured { generation: 3 },
            Message::Terminate {
                generation: 3,
                id: 1,
            },
            Message::Consumed { id: 1 },
        ];
        for message in messages {
            let bytes = message.encode();
            assert_eq!(Message::decode(&bytes), Some(message));
            for length in 0..BYTES {
                assert_eq!(Message::decode(&bytes[..length]), None);
            }
            let mut oversized = [0; BYTES + 1];
            oversized[..BYTES].copy_from_slice(&bytes);
            assert_eq!(Message::decode(&oversized), None);
            let mut invalid = bytes;
            invalid[8] = 255;
            assert_eq!(Message::decode(&invalid), None);
        }
        let mut invalid = Message::Configure {
            generation: 1,
            disposition: Disposition::Notify,
        }
        .encode();
        invalid[12] = 3;
        assert_eq!(Message::decode(&invalid), None);
        let mut invalid = Message::Configured { generation: 1 }.encode();
        invalid[24] = 1;
        assert_eq!(Message::decode(&invalid), None);
    }

    #[test]
    fn generation_and_delivery_ids_are_nonzero() {
        for message in [
            Message::Configure {
                generation: 0,
                disposition: Disposition::Notify,
            },
            Message::Configured { generation: 0 },
            Message::Terminate {
                generation: 0,
                id: 1,
            },
            Message::Terminate {
                generation: 1,
                id: 0,
            },
            Message::Consumed { id: 0 },
        ] {
            assert_eq!(Message::decode(&message.encode()), None);
        }
    }
}

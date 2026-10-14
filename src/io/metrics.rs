// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Guest-reported usage, distinct from hypervisor-owned RAM accounting.

use serde::{Deserialize, Serialize};

pub const WIRE_BYTES: usize = 88;

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum AgentState {
    #[default]
    Unavailable,
    Sampling,
    Ready,
    Stale,
    Unsupported,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct CpuUsage {
    /// Percentage of all guest CPUs, in hundredths of one percent (0..=10000).
    pub busy_basis_points: u32,
    pub iowait_basis_points: u32,
    pub interval_ms: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub struct MemoryUsage {
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub free_bytes: u64,
    pub buffers_bytes: u64,
    pub cached_bytes: u64,
    pub reclaimable_slab_bytes: u64,
}

impl MemoryUsage {
    pub fn valid(self) -> bool {
        self.total_bytes != 0
            && [
                self.available_bytes,
                self.free_bytes,
                self.buffers_bytes,
                self.cached_bytes,
                self.reclaimable_slab_bytes,
            ]
            .into_iter()
            .all(|bytes| bytes <= self.total_bytes)
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct GuestMetrics {
    pub state: AgentState,
    /// Age of the last complete sample, measured by the Native runtime.
    pub sample_age_ms: Option<u64>,
    pub cpu: Option<CpuUsage>,
    pub memory: Option<MemoryUsage>,
}

impl GuestMetrics {
    pub fn encode(self) -> Option<[u8; WIRE_BYTES]> {
        if !self.valid() {
            return None;
        }
        let mut bytes = [0; WIRE_BYTES];
        bytes[0] = match self.state {
            AgentState::Unavailable => 0,
            AgentState::Sampling => 1,
            AgentState::Ready => 2,
            AgentState::Stale => 3,
            AgentState::Unsupported => 4,
        };
        bytes[1] = u8::from(self.cpu.is_some());
        bytes[2] = u8::from(self.memory.is_some());
        bytes[8..16].copy_from_slice(&self.sample_age_ms.unwrap_or(u64::MAX).to_le_bytes());
        if let Some(cpu) = self.cpu {
            bytes[16..20].copy_from_slice(&cpu.busy_basis_points.to_le_bytes());
            bytes[20..24].copy_from_slice(&cpu.iowait_basis_points.to_le_bytes());
            bytes[24..32].copy_from_slice(&cpu.interval_ms.to_le_bytes());
        }
        if let Some(memory) = self.memory {
            for (slot, value) in bytes[32..80].chunks_exact_mut(8).zip([
                memory.total_bytes,
                memory.available_bytes,
                memory.free_bytes,
                memory.buffers_bytes,
                memory.cached_bytes,
                memory.reclaimable_slab_bytes,
            ]) {
                slot.copy_from_slice(&value.to_le_bytes());
            }
        }
        Some(bytes)
    }

    pub fn decode(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != WIRE_BYTES
            || bytes[1] > 1
            || bytes[2] > 1
            || bytes[3..8] != [0; 5]
            || bytes[80..] != [0; 8]
        {
            return None;
        }
        let word = |offset| {
            Some(u64::from_le_bytes(
                bytes.get(offset..offset + 8)?.try_into().ok()?,
            ))
        };
        let state = match bytes[0] {
            0 => AgentState::Unavailable,
            1 => AgentState::Sampling,
            2 => AgentState::Ready,
            3 => AgentState::Stale,
            4 => AgentState::Unsupported,
            _ => return None,
        };
        let age = word(8)?;
        let value = Self {
            state,
            sample_age_ms: (age != u64::MAX).then_some(age),
            cpu: if bytes[1] == 1 {
                Some(CpuUsage {
                    busy_basis_points: u32::from_le_bytes(bytes[16..20].try_into().ok()?),
                    iowait_basis_points: u32::from_le_bytes(bytes[20..24].try_into().ok()?),
                    interval_ms: word(24)?,
                })
            } else {
                if bytes[16..32] != [0; 16] {
                    return None;
                }
                None
            },
            memory: if bytes[2] == 1 {
                Some(MemoryUsage {
                    total_bytes: word(32)?,
                    available_bytes: word(40)?,
                    free_bytes: word(48)?,
                    buffers_bytes: word(56)?,
                    cached_bytes: word(64)?,
                    reclaimable_slab_bytes: word(72)?,
                })
            } else {
                if bytes[32..80] != [0; 48] {
                    return None;
                }
                None
            },
        };
        value.valid().then_some(value)
    }

    pub fn valid(self) -> bool {
        if self.sample_age_ms == Some(u64::MAX)
            || self.memory.is_some_and(|memory| !memory.valid())
            || self.cpu.is_some_and(|cpu| {
                cpu.busy_basis_points > 10000
                    || cpu.iowait_basis_points > 10000 - cpu.busy_basis_points
                    || cpu.interval_ms == 0
            })
        {
            return false;
        }
        match self.state {
            AgentState::Ready => {
                self.cpu.is_some() && self.memory.is_some() && self.sample_age_ms.is_some()
            }
            AgentState::Sampling => {
                self.cpu.is_none() && (self.memory.is_some() == self.sample_age_ms.is_some())
            }
            AgentState::Stale => {
                self.cpu.is_none() && self.memory.is_none() && self.sample_age_ms.is_some()
            }
            AgentState::Unavailable | AgentState::Unsupported => {
                self.cpu.is_none() && self.memory.is_none() && self.sample_age_ms.is_none()
            }
        }
    }
}

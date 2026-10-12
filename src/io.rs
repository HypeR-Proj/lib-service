// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Explicit startup authority for the trusted physical I/O VM owner.

use crate::{StartupContract, vm};
use hyper_os::{handle::Rights, startup};

mod binding;
mod connection;

pub use binding::{BOUND_BYTES, BOUND_MESSAGE, Binding, NETWORK_DEVICE_ID_BIT};
pub use connection::{
    CONNECT_BYTES, Connection, DISK, NETWORK, decode_connect, encode_connect, parse_mac,
    valid_name, valid_volume,
};

pub const DEVICE_AUTHORITY_NAME: &str = "io.device-authority";
pub const STARTUP_CONTRACTS: &[StartupContract] = &[
    vm::MANAGER_CREATION_AUTHORITY_CONTRACT,
    READY_CONTRACT,
    BROKER_SERVER_CONTRACT,
    crate::filesystem::PROVIDER_CONTRACT,
    StartupContract::exact(
        DEVICE_AUTHORITY_NAME,
        startup::DEVICE_ASSIGNMENT_AUTHORITY,
        Rights::INSPECT,
    ),
];

/// Explicit one-shot result for bootstrap mounts, before formal init takes over.
pub const READY_NAME: &str = "io.ready";
pub const READY: startup::StartupPurpose<hyper_os::handle::ByteChannelObject> =
    startup::StartupPurpose::new(0x8006_0001);
pub const READY_CONTRACT: StartupContract =
    StartupContract::exact(READY_NAME, READY, Rights::WAIT.union(Rights::WRITE));
pub const READY_MESSAGE: &[u8] = b"HYPER-IO-READY/1\n";
pub const READY_TIMEOUT_SECONDS: u64 = 120;

/// The trusted manager delegates individual runtime sessions over this channel.
pub const BROKER_SERVER: startup::StartupPurpose<hyper_os::handle::CapabilityChannelObject> =
    startup::StartupPurpose::new(0x8006_0002);
pub const BROKER_CLIENT: startup::StartupPurpose<hyper_os::handle::CapabilityChannelObject> =
    startup::StartupPurpose::new(0x8006_0003);
pub const SESSION: startup::StartupPurpose<hyper_os::handle::CapabilityChannelObject> =
    startup::StartupPurpose::new(0x8006_0004);
pub const BROKER_SERVER_CONTRACT: StartupContract = StartupContract::exact(
    "io.broker-server",
    BROKER_SERVER,
    Rights::WAIT.union(Rights::READ),
);
pub const BROKER_CLIENT_CONTRACT: StartupContract = StartupContract::exact(
    "io.broker-client",
    BROKER_CLIENT,
    Rights::WAIT.union(Rights::WRITE),
);
pub const SESSION_RIGHTS: Rights = Rights::WAIT.union(Rights::READ).union(Rights::WRITE);
pub const SESSION_CONTRACT: StartupContract =
    StartupContract::exact("io.session", SESSION, SESSION_RIGHTS);

/// Capability rendezvous records have exact lengths and no native-layout fields.
pub const MEMORY_BYTES: usize = 24;
pub const FRONTEND_MMIO: u64 = 0x0a00_0000;
pub const FRONTEND_IRQ: u32 = 48;
/// Client zero belongs to Native services. Its network route remains dormant
/// until a Native network consumer is implemented; guests cannot claim it.
pub const NATIVE_CLIENT: u32 = 0;
pub const NETWORK_NOTIFICATION_BASE: u64 = 0x0a20_0000;
pub const NETWORK_NOTIFICATION_STRIDE: u64 = 0x10000;
pub const NETWORK_NOTIFICATION_IRQ_BASE: u32 = 64;
pub const FRONTEND_NET_MMIO: u64 = 0x0a00_1000;
pub const FRONTEND_NET_IRQ: u32 = 49;
pub const MAILBOX_RIGHTS: Rights = Rights::WAIT.union(Rights::READ).union(Rights::WRITE);
pub const NOTIFICATION_RIGHTS: Rights = Rights::INSPECT.union(Rights::WRITE).union(Rights::WAIT);
pub const FRONTEND_RIGHTS: Rights = Rights::INSPECT.union(Rights::WRITE).union(Rights::WAIT);
pub const MEMORY_RIGHTS: Rights = Rights::INSPECT.union(Rights::MAP);

pub fn encode_memory(base: u64, length: u64) -> Option<[u8; MEMORY_BYTES]> {
    if length == 0 || (base | length) & 4095 != 0 || base.checked_add(length).is_none() {
        return None;
    }
    let mut bytes = [0; MEMORY_BYTES];
    bytes[..8].copy_from_slice(b"HIOMEM01");
    bytes[8..16].copy_from_slice(&base.to_le_bytes());
    bytes[16..24].copy_from_slice(&length.to_le_bytes());
    Some(bytes)
}
pub fn decode_memory(bytes: &[u8]) -> Option<(u64, u64)> {
    if bytes.len() != MEMORY_BYTES || &bytes[..8] != b"HIOMEM01" {
        return None;
    }
    let base = u64::from_le_bytes(bytes[8..16].try_into().ok()?);
    let length = u64::from_le_bytes(bytes[16..24].try_into().ok()?);
    encode_memory(base, length).map(|_| (base, length))
}

/// Rendezvous has no queued capability ownership; the caller keeps every MOVE
/// source until the exact receive commits, and bounds waiting with one deadline.
pub fn send_capabilities(
    endpoint: &hyper_os::capability_channel::CapabilityChannel,
    bytes: &[u8],
    dispositions: &mut [hyper_os::capability_channel::CapabilityDisposition<'_>],
    deadline: u64,
) -> hyper_os::Result<()> {
    use hyper_os::handle::CapabilityChannelObject;
    use hyper_os::wait::{ObjectSignals, WaitItem, wait_many};
    loop {
        let observation = wait_many(
            &[WaitItem::new(
                endpoint.as_handle_ref(),
                ObjectSignals::<CapabilityChannelObject>::PEER_RECEIVING
                    .union(ObjectSignals::<CapabilityChannelObject>::PEER_CLOSED),
            )],
            deadline,
        )?;
        if !ObjectSignals::<CapabilityChannelObject>::PEER_RECEIVING
            .is_present_in(observation.observed)
        {
            return Err(hyper_os::Error::InvalidResponse);
        }
        match endpoint.try_send(bytes, dispositions) {
            Err(hyper_os::Error::Status(hyper_os::Status::WOULD_BLOCK)) => {}
            result => return result,
        }
    }
}

/// Read-only broker observation; no VM/control capability crosses this exchange.
pub const OBSERVE_MESSAGE: &[u8] = b"HIOSTAT3";
pub const OBSERVATION_BYTES: usize = 584;
/// Identity admission can wait behind synchronous guest-mapping retirement.
/// Best-effort status callers may use a shorter deadline on their reply channel.
pub const OBSERVATION_TIMEOUT_SECONDS: u64 = 5;

/// RAM is supplied by the owner: the VM address-space span also includes MMIO
/// and shared guest-memory windows, and is not a resident-memory statistic.
/// Placement observes vCPU 0 of the current single-vCPU I/O service. A missing
/// CPU means unavailable, never an assumed CPU 0 assignment.
pub fn encode_observation(
    name: &str,
    image: &str,
    info: hyper_os::vm::VirtualMachineInfo,
    ram_bytes: u64,
    boot_host_cpu: Option<u32>,
) -> Option<[u8; OBSERVATION_BYTES]> {
    use hyper_os::vm::VirtualMachinePhase;
    if !valid_observed_name(name) || !valid_observed_image(image) {
        return None;
    }
    let mut bytes = [0; OBSERVATION_BYTES];
    bytes[..8].copy_from_slice(OBSERVE_MESSAGE);
    bytes[8] = match info.phase {
        VirtualMachinePhase::Installed => 0,
        VirtualMachinePhase::Running => 1,
        VirtualMachinePhase::Stopping => 2,
        VirtualMachinePhase::Stopped => 3,
    };
    bytes[12..16].copy_from_slice(&info.vcpu_count.to_le_bytes());
    bytes[16..24].copy_from_slice(&ram_bytes.to_le_bytes());
    bytes[24..32].copy_from_slice(&info.resident_memory_bytes.unwrap_or(u64::MAX).to_le_bytes());
    bytes[32..36].copy_from_slice(&boot_host_cpu.unwrap_or(u32::MAX).to_le_bytes());
    bytes[36] = name.len() as u8;
    bytes[38..40].copy_from_slice(&(image.len() as u16).to_le_bytes());
    bytes[40..40 + name.len()].copy_from_slice(name.as_bytes());
    bytes[72..72 + image.len()].copy_from_slice(image.as_bytes());
    Some(bytes)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Observation<'a> {
    /// Owner-supplied identity, immutable for the lifetime of the broker endpoint.
    pub name: &'a str,
    pub image: &'a str,
    pub phase: hyper_os::vm::VirtualMachinePhase,
    pub vcpus: u32,
    pub ram_bytes: u64,
    pub resident_bytes: Option<u64>,
    /// Current assignment of the I/O VM's boot vCPU (vCPU 0).
    pub boot_host_cpu: Option<u32>,
}

pub fn decode_observation(bytes: &[u8]) -> Option<Observation<'_>> {
    use hyper_os::vm::VirtualMachinePhase;
    if bytes.len() != OBSERVATION_BYTES
        || &bytes[..8] != OBSERVE_MESSAGE
        || bytes[9..12] != [0; 3]
        || bytes[37] != 0
    {
        return None;
    }
    let name = padded_text(&bytes[40..72], usize::from(bytes[36]))?;
    let image_length = usize::from(u16::from_le_bytes(bytes[38..40].try_into().ok()?));
    let image = padded_text(&bytes[72..], image_length)?;
    if !valid_observed_name(name) || !valid_observed_image(image) {
        return None;
    }
    let phase = match bytes[8] {
        0 => VirtualMachinePhase::Installed,
        1 => VirtualMachinePhase::Running,
        2 => VirtualMachinePhase::Stopping,
        3 => VirtualMachinePhase::Stopped,
        _ => return None,
    };
    Some(Observation {
        name,
        image,
        phase,
        vcpus: u32::from_le_bytes(bytes[12..16].try_into().ok()?),
        ram_bytes: u64::from_le_bytes(bytes[16..24].try_into().ok()?),
        resident_bytes: match u64::from_le_bytes(bytes[24..32].try_into().ok()?) {
            u64::MAX => None,
            bytes => Some(bytes),
        },
        boot_host_cpu: match u32::from_le_bytes(bytes[32..36].try_into().ok()?) {
            u32::MAX => None,
            cpu => Some(cpu),
        },
    })
}

fn padded_text(bytes: &[u8], length: usize) -> Option<&str> {
    let (text, padding) = bytes.split_at_checked(length)?;
    if padding.iter().any(|byte| *byte != 0) {
        return None;
    }
    core::str::from_utf8(text).ok()
}

fn valid_observed_image(image: &str) -> bool {
    image.starts_with('/')
        && image.len() <= 512
        && !image.chars().any(char::is_control)
        && !image
            .split('/')
            .skip(1)
            .any(|part| matches!(part, "" | "." | ".."))
}

fn valid_observed_name(name: &str) -> bool {
    // Same namespace as fleet definitions; reject ambiguous or unprintable wire names.
    !name.is_empty()
        && name.len() <= 32
        && name.as_bytes()[0].is_ascii_alphanumeric()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte))
}

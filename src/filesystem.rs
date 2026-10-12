// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Volume attachment and lifecycle contracts for Native filesystem services.

pub mod control;

use crate::StartupContract;
use hyper_os::handle::{ByteChannelObject, CapabilityChannelObject, NativeBlockObject, Rights};
use hyper_os::startup::StartupPurpose;

/// Init connects one volume provider to the resident filesystem manager.
pub const ATTACH: StartupPurpose<CapabilityChannelObject> = StartupPurpose::new(0x8007_0001);
pub const PROVIDER: StartupPurpose<CapabilityChannelObject> = StartupPurpose::new(0x8007_0002);
pub const ATTACH_CONTRACT: StartupContract = StartupContract::exact(
    "filesystem.attach",
    ATTACH,
    Rights::WAIT.union(Rights::READ),
);
pub const PROVIDER_CONTRACT: StartupContract = StartupContract::exact(
    "filesystem.provider",
    PROVIDER,
    Rights::WAIT.union(Rights::WRITE),
);
pub const CONTROL_SERVER: StartupPurpose<CapabilityChannelObject> =
    StartupPurpose::new(0x8007_0006);
pub const CONTROL_CLIENT: StartupPurpose<CapabilityChannelObject> =
    StartupPurpose::new(0x8007_0007);
pub const WORKER_CONTROL: StartupPurpose<CapabilityChannelObject> =
    StartupPurpose::new(0x8007_0008);
pub const CONTROL_SERVER_CONTRACT: StartupContract = StartupContract::exact(
    "filesystem.control-server",
    CONTROL_SERVER,
    Rights::WAIT.union(Rights::READ),
);
pub const CONTROL_CLIENT_CONTRACT: StartupContract = StartupContract::exact(
    "filesystem.control-client",
    CONTROL_CLIENT,
    Rights::WAIT.union(Rights::WRITE),
);
/// Session and shell receive a delegatable client; leaf mount tools receive
/// only `CONTROL_CLIENT_CONTRACT` and cannot pass this authority onward.
pub const DELEGATED_CONTROL_CLIENT_CONTRACT: StartupContract = StartupContract::exact(
    "filesystem.control-client",
    CONTROL_CLIENT,
    Rights::WAIT
        .union(Rights::WRITE)
        .union(Rights::DUPLICATE)
        .union(Rights::TRANSFER),
);
pub const MANAGER_STARTUP_CONTRACTS: &[StartupContract] =
    &[ATTACH_CONTRACT, CONTROL_SERVER_CONTRACT];

/// Only the selected worker receives volume authority. The owner pipe makes
/// manager termination observable without a syscall on each filesystem read.
pub const BLOCK: StartupPurpose<NativeBlockObject> = StartupPurpose::new(0x8007_0003);
pub const OWNER: StartupPurpose<ByteChannelObject> = StartupPurpose::new(0x8007_0005);

pub const ATTACH_BYTES: usize = 160;
pub const BEGIN_MESSAGE: &[u8] = b"HFSBEG01";
pub const FINISH_MESSAGE: &[u8] = b"HFSEND01";
pub const READY_MESSAGE: &[u8] = b"HYPER-FS-READY/1\n";
pub const FAILURE_MESSAGE: &[u8] = b"HYPER-FS-FAILED/1\n";
pub const WORKER_READY: &[u8] = b"HFSWRK01";
pub const WORKER_RETURN: &[u8] = b"HFSRET01";
pub const WORKER_STOP: &[u8] = b"HFSSTOP1";
pub const CONTROL_CONNECT: &[u8] = b"HFSMNT01";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Attachment<'a> {
    pub id: hyper_block::Guid,
    pub label: &'a str,
    pub volume: hyper_os::block::VolumeInfo,
}

pub fn valid_volume_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 31
        && name.as_bytes()[0].is_ascii_lowercase()
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// BEGIN transfers a readiness writer, then each attachment moves one exclusive
/// activated `NativeBlock`. FINISH transfers no handles and starts bootstrap fstab mounting.
pub fn encode_attach(
    id: hyper_block::Guid,
    label: &str,
    volume: hyper_os::block::VolumeInfo,
) -> Option<[u8; ATTACH_BYTES]> {
    if volume.sectors == 0
        || id == hyper_block::Guid::NIL
        || label.encode_utf16().count() > 36
        || label.contains('\0')
    {
        return None;
    }
    let mut bytes = [0; ATTACH_BYTES];
    bytes[..8].copy_from_slice(b"HFSVOL03");
    bytes[8..16].copy_from_slice(&volume.sectors.to_le_bytes());
    bytes[16..24].copy_from_slice(&u64::from(volume.read_only).to_le_bytes());
    bytes[24..40].copy_from_slice(&id.0);
    bytes[40..40 + label.len()].copy_from_slice(label.as_bytes());
    Some(bytes)
}

pub fn decode_attach(bytes: &[u8]) -> Option<Attachment<'_>> {
    if bytes.len() != ATTACH_BYTES || &bytes[..8] != b"HFSVOL03" {
        return None;
    }
    let volume = hyper_os::block::VolumeInfo {
        sectors: u64::from_le_bytes(bytes[8..16].try_into().ok()?),
        read_only: u64::from_le_bytes(bytes[16..24].try_into().ok()?) != 0,
    };
    let id = hyper_block::Guid(bytes[24..40].try_into().ok()?);
    let end = bytes[40..].iter().position(|byte| *byte == 0)? + 40;
    let label = core::str::from_utf8(&bytes[40..end]).ok()?;
    (encode_attach(id, label, volume)?.as_slice() == bytes).then_some(Attachment {
        id,
        label,
        volume,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn attachment_preserves_partition_identity_and_rejects_hidden_fields()
    -> Result<(), &'static str> {
        let volume = hyper_os::block::VolumeInfo {
            sectors: 65536,
            read_only: true,
        };
        let id = hyper_block::Guid([1; 16]);
        let mut bytes = encode_attach(id, "系统", volume).ok_or("encoding failed")?;
        assert_eq!(
            decode_attach(&bytes),
            Some(Attachment {
                id,
                label: "系统",
                volume
            })
        );
        assert!(encode_attach(hyper_block::Guid::NIL, "system", volume).is_none());
        assert!(decode_attach(&bytes[..20]).is_none());
        bytes[16] = 2;
        assert!(decode_attach(&bytes).is_none());
        bytes[16] = 1;
        bytes[159] = 1;
        assert!(decode_attach(&bytes).is_none());
        Ok(())
    }
}

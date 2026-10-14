// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use hyper_os::vm::{Architecture, PlatformProfile, VirtualMachineInfo, VirtualMachinePhase};
use hyper_service::io::metrics::{AgentState, CpuUsage, GuestMetrics, MemoryUsage};
use hyper_service::io::{decode_observation, encode_observation};

#[test]
fn guest_usage_wire_rejects_inconsistent_or_out_of_range_samples() {
    let sample = GuestMetrics {
        state: AgentState::Ready,
        sample_age_ms: Some(17),
        cpu: Some(CpuUsage {
            busy_basis_points: 1250,
            iowait_basis_points: 250,
            interval_ms: 5000,
        }),
        memory: Some(MemoryUsage {
            total_bytes: 1024,
            available_bytes: 512,
            free_bytes: 128,
            buffers_bytes: 64,
            cached_bytes: 256,
            reclaimable_slab_bytes: 32,
        }),
    };
    let wire = sample.encode().unwrap();
    assert_eq!(GuestMetrics::decode(&wire), Some(sample));
    for offset in [3, 7, 80, 87] {
        let mut invalid = wire;
        invalid[offset] = 1;
        assert_eq!(GuestMetrics::decode(&invalid), None);
    }
    let mut invalid = wire;
    invalid[16..20].copy_from_slice(&10001u32.to_le_bytes());
    assert_eq!(GuestMetrics::decode(&invalid), None);
    invalid = wire;
    invalid[40..48].copy_from_slice(&1025u64.to_le_bytes());
    assert_eq!(GuestMetrics::decode(&invalid), None);
    for state in [
        AgentState::Unavailable,
        AgentState::Sampling,
        AgentState::Stale,
        AgentState::Unsupported,
    ] {
        assert!(GuestMetrics { state, ..sample }.encode().is_none());
    }
    for sample in [
        GuestMetrics::default(),
        GuestMetrics {
            state: AgentState::Unsupported,
            ..GuestMetrics::default()
        },
        GuestMetrics {
            state: AgentState::Sampling,
            cpu: None,
            ..sample
        },
        GuestMetrics {
            state: AgentState::Stale,
            sample_age_ms: Some(16000),
            ..GuestMetrics::default()
        },
    ] {
        assert_eq!(
            GuestMetrics::decode(&sample.encode().unwrap()),
            Some(sample)
        );
    }
}

#[test]
fn observations_validate_wire_format_and_preserve_metrics() -> Result<(), &'static str> {
    for phase in [
        VirtualMachinePhase::Installed,
        VirtualMachinePhase::Running,
        VirtualMachinePhase::Stopping,
        VirtualMachinePhase::Stopped,
    ] {
        let info = VirtualMachineInfo {
            phase,
            vcpu_count: 4,
            guest_physical_base: 0x4000_0000,
            memory_size: 512 * 1024 * 1024,
            resident_memory_bytes: Some(65 * 1024 * 1024),
            architecture: Architecture::Aarch64,
            platform_profile: PlatformProfile::Aarch64Reference,
        };
        let bytes = encode_observation(
            "storage-backend",
            "/vm/storage/oi.itb",
            info,
            64 * 1024 * 1024,
            Some(3),
            Default::default(),
        )
        .ok_or("valid observation rejected")?;
        assert_eq!(
            decode_observation(&bytes),
            Some(hyper_service::io::Observation {
                name: "storage-backend",
                image: "/vm/storage/oi.itb",
                phase,
                vcpus: 4,
                ram_bytes: 64 * 1024 * 1024,
                resident_bytes: Some(65 * 1024 * 1024),
                boot_host_cpu: Some(3),
                metrics: Default::default(),
            })
        );
        assert_eq!(decode_observation(&bytes[..23]), None);
        let mut invalid = bytes;
        invalid[8] = 4;
        assert_eq!(decode_observation(&invalid), None);
        invalid = bytes;
        invalid[9] = 1;
        assert_eq!(decode_observation(&invalid), None);
        invalid = bytes;
        invalid[36] = 1;
        assert_eq!(decode_observation(&invalid), None);
        invalid = bytes;
        invalid[7] = b'1';
        assert_eq!(decode_observation(&invalid), None);
        for length in [0, 33, 255] {
            invalid = bytes;
            invalid[36] = length;
            assert_eq!(decode_observation(&invalid), None);
        }
        for byte in [0, 0xff, b'/', b' '] {
            invalid = bytes;
            invalid[40] = byte;
            assert_eq!(decode_observation(&invalid), None);
        }
        invalid = bytes;
        invalid[71] = b'x';
        assert_eq!(decode_observation(&invalid), None);
        for name in [
            "",
            "-invalid",
            "bad/name",
            "bad name",
            "a\0b",
            "abcdefghijklmnopqrstuvwxyz1234567",
        ] {
            assert!(
                encode_observation(
                    name,
                    "/vm/storage/oi.itb",
                    info,
                    0,
                    None,
                    Default::default()
                )
                .is_none()
            );
        }
        let longest = "abcdefghijklmnopqrstuvwxyz123456";
        let maximum = encode_observation(
            longest,
            "/vm/storage/oi.itb",
            info,
            0,
            None,
            Default::default(),
        )
        .ok_or("valid observation rejected")?;
        assert_eq!(
            decode_observation(&maximum).map(|info| info.name),
            Some(longest)
        );
        for length in [0u16, 513, u16::MAX] {
            invalid = bytes;
            invalid[38..40].copy_from_slice(&length.to_le_bytes());
            assert_eq!(decode_observation(&invalid), None);
        }
        for byte in [0, 0xff, b'x'] {
            invalid = bytes;
            invalid[72] = byte;
            assert_eq!(decode_observation(&invalid), None);
        }
        invalid = bytes;
        invalid[37] = 1;
        assert_eq!(decode_observation(&invalid), None);
        invalid = bytes;
        invalid[583] = 1;
        assert_eq!(decode_observation(&invalid), None);
        for path in [
            "",
            "vm/oi.itb",
            "/vm/../oi.itb",
            "/vm/./oi.itb",
            "/vm//oi.itb",
            "/vm/oi\nitb",
        ] {
            assert!(encode_observation("oi", path, info, 0, None, Default::default()).is_none());
        }
        let long_path = format!("/vm/{}", "a".repeat(508));
        let longest_image = encode_observation("oi", &long_path, info, 0, None, Default::default())
            .ok_or("maximum length image path rejected")?;
        assert_eq!(
            decode_observation(&longest_image).map(|info| info.image),
            Some(long_path.as_str())
        );
        assert!(
            encode_observation("oi", &(long_path + "a"), info, 0, None, Default::default())
                .is_none()
        );
        let unavailable = encode_observation(
            "storage-backend",
            "/vm/storage/oi.itb",
            info,
            64 * 1024 * 1024,
            None,
            Default::default(),
        )
        .ok_or("valid observation rejected")?;
        assert_eq!(
            decode_observation(&unavailable).map(|info| info.boot_host_cpu),
            Some(None)
        );
        let cpu_zero = encode_observation(
            "storage-backend",
            "/vm/storage/oi.itb",
            info,
            64 * 1024 * 1024,
            Some(0),
            Default::default(),
        )
        .ok_or("valid observation rejected")?;
        assert_eq!(
            decode_observation(&cpu_zero).map(|info| info.boot_host_cpu),
            Some(Some(0))
        );
    }
    Ok(())
}

// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Shared, typed contracts between `HypeR` Native system services and clients.

#![no_std]

pub mod console;
pub mod filesystem;
pub mod io;
pub mod process;
pub mod session;
pub mod stdio;
pub mod vm;

use hyper_os::handle::{Rights, TypedObject};
use hyper_os::startup::StartupPurpose;

/// One typed startup capability contract shared by a service and init.
///
/// Construction is generic over the object marker, so a purpose cannot be
/// paired with the wrong Native object kind. The erased descriptor remains a
/// small value which policy code can match without retaining a live handle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StartupContract {
    name: &'static str,
    purpose: u32,
    object_kind: u32,
    required_rights: Rights,
    allowed_rights: Rights,
}

impl StartupContract {
    #[must_use]
    pub const fn exact<T: TypedObject>(
        name: &'static str,
        purpose: StartupPurpose<T>,
        rights: Rights,
    ) -> Self {
        Self {
            name,
            purpose: purpose.as_raw(),
            object_kind: T::KIND.as_raw(),
            required_rights: rights,
            allowed_rights: rights,
        }
    }

    /// Allows explicit attenuation between required and optional authority.
    /// Optional rights are a ceiling, never an implicit startup grant.
    #[must_use]
    pub const fn with_optional_rights(mut self, rights: Rights) -> Self {
        self.allowed_rights = self.allowed_rights.union(rights);
        self
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        self.name
    }

    #[must_use]
    pub const fn purpose(self) -> u32 {
        self.purpose
    }

    #[must_use]
    pub const fn object_kind(self) -> u32 {
        self.object_kind
    }

    #[must_use]
    pub const fn required_rights(self) -> Rights {
        self.required_rights
    }

    #[must_use]
    pub const fn allowed_rights(self) -> Rights {
        self.allowed_rights
    }
}

#[cfg(test)]
mod tests {
    use hyper_os::handle::{ConsoleObject, Rights, TypedObject};
    use hyper_os::startup::StartupPurpose;

    use super::StartupContract;

    fn assert_unique_contracts(first: &[StartupContract], second: &[StartupContract]) {
        for (left_index, left) in first.iter().enumerate() {
            for right in first.iter().skip(left_index + 1).chain(second.iter()) {
                assert_ne!(left.name(), right.name());
                assert_ne!(left.purpose(), right.purpose());
            }
        }
        for (left_index, left) in second.iter().enumerate() {
            for right in second.iter().skip(left_index + 1) {
                assert_ne!(left.name(), right.name());
                assert_ne!(left.purpose(), right.purpose());
            }
        }
    }

    #[test]
    fn exact_contract_preserves_typed_purpose_and_rights() {
        let purpose = StartupPurpose::<ConsoleObject>::new(0x8123_0001);
        let rights = Rights::WAIT.union(Rights::WRITE);
        let contract = StartupContract::exact("test.console", purpose, rights);

        assert_eq!(contract.name(), "test.console");
        assert_eq!(contract.purpose(), purpose.as_raw());
        assert_eq!(contract.object_kind(), ConsoleObject::KIND.as_raw());
        assert_eq!(contract.required_rights(), rights);
        assert_eq!(contract.allowed_rights(), rights);
    }

    #[test]
    fn composed_service_contracts_are_unambiguous() {
        assert_unique_contracts(
            super::stdio::APPLICATION_STARTUP_CONTRACTS,
            super::process::APPLICATION_STARTUP_CONTRACTS,
        );
        assert_unique_contracts(super::console::INPUT_STARTUP_CONTRACTS, &[]);
        assert_unique_contracts(super::console::OUTPUT_STARTUP_CONTRACTS, &[]);
        assert_unique_contracts(super::session::STARTUP_CONTRACTS, &[]);
        assert_unique_contracts(
            super::stdio::STARTUP_CONTRACTS,
            super::process::SHELL_STARTUP_CONTRACTS,
        );
        assert_unique_contracts(
            super::vm::MANAGER_STARTUP_CONTRACTS,
            super::process::VM_MANAGER_STARTUP_CONTRACTS,
        );
    }
}

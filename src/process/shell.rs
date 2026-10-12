// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Authority retained by an interactive shell for launching trusted tools.

use hyper_os::handle::{
    CapabilityChannelObject, CpuInspectorObject, DirectoryObject, FileObject, HandleRef,
    MemoryInspectorObject, ObjectInspectorObject, RightsOffer, TaskInspectorObject,
};
use hyper_os::task::ProcessBuilder;

/// Shell-specific grants shared by session and nested shell launchers.
///
/// The caller first selects the trusted shell image, then delegates its usual
/// root, task construction and stdio capabilities independently. Ordinary
/// commands must not receive this inventory. The loader and library directory
/// always describe the retained bootstrap environment, even when a dynamic
/// loader starts the formal shell itself.
pub struct ShellDelegation<'a> {
    pub environment: HandleRef<'a, DirectoryObject>,
    pub loader: HandleRef<'a, FileObject>,
    pub libraries: HandleRef<'a, DirectoryObject>,
    pub tasks: HandleRef<'a, TaskInspectorObject>,
    pub objects: HandleRef<'a, ObjectInspectorObject>,
    pub memory: HandleRef<'a, MemoryInspectorObject>,
    pub cpus: HandleRef<'a, CpuInspectorObject>,
    pub filesystem: Option<HandleRef<'a, CapabilityChannelObject>>,
    pub virtual_machines: Option<HandleRef<'a, CapabilityChannelObject>>,
}

impl ShellDelegation<'_> {
    pub fn add_to(&self, builder: &ProcessBuilder) -> hyper_os::Result<()> {
        macro_rules! grant {
            ($handle:expr, $contract:expr) => {
                builder.add_handle_duplicate(
                    $handle,
                    $contract.purpose(),
                    RightsOffer::Exact($contract.allowed_rights()),
                )?;
            };
        }
        grant!(self.environment, super::ENVIRONMENT_DIRECTORY_CONTRACT);
        grant!(self.loader, super::CHILD_LOADER_CONTRACT);
        grant!(self.libraries, super::CHILD_LIBRARY_DIRECTORY_CONTRACT);
        grant!(self.tasks, super::SHELL_TASK_INSPECTOR_CONTRACT);
        grant!(self.objects, super::SHELL_OBJECT_INSPECTOR_CONTRACT);
        grant!(self.memory, super::SHELL_MEMORY_INSPECTOR_CONTRACT);
        grant!(self.cpus, super::SHELL_CPU_INSPECTOR_CONTRACT);
        if let Some(handle) = self.filesystem {
            grant!(handle, crate::filesystem::DELEGATED_CONTROL_CLIENT_CONTRACT);
        }
        if let Some(handle) = self.virtual_machines {
            grant!(handle, crate::vm::DELEGATED_CLIENT_CONNECTION_CONTRACT);
        }
        Ok(())
    }
}

// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Symbolic manifest names for standard process-construction authorities.

use hyper_os::handle::{DirectoryObject, Rights};
use hyper_os::startup::{self, StartupPurpose};

use crate::StartupContract;

mod shell;
pub use shell::ShellDelegation;

pub const ROOT_DIRECTORY_NAME: &str = "process.root-directory";
pub const TASK_FACTORY_NAME: &str = "process.task-factory";
pub const TASK_GROUP_NAME: &str = "process.task-group";
pub const RESOURCE_DOMAIN_NAME: &str = "process.resource-domain";
pub const TASK_INSPECTOR_NAME: &str = "process.task-inspector";
pub const OBJECT_INSPECTOR_NAME: &str = "process.object-inspector";
pub const MEMORY_INSPECTOR_NAME: &str = "process.memory-inspector";
pub const CPU_INSPECTOR_NAME: &str = "process.cpu-inspector";
pub const CHILD_LOADER_NAME: &str = "process.child-loader";
pub use hyper_os::startup::CHILD_LOADER;

pub const CHILD_LIBRARY_DIRECTORY_NAME: &str = "process.child-library-directory";
pub const ENVIRONMENT_DIRECTORY_NAME: &str = "process.environment-directory";

/// Authority passed to `ProcessBuilder` for an ordinary executable file.
/// The builder copies READ/EXECUTE/PROTECT into the child loader and consumes
/// neither this caller's File handle nor its propagation rights.
pub const EXECUTABLE_FILE_RIGHTS: hyper_os::fs::FileRights = hyper_os::fs::FileRights::READ
    .union(hyper_os::fs::FileRights::EXECUTE)
    .union(hyper_os::fs::FileRights::PROTECT)
    .union(hyper_os::fs::FileRights::DUPLICATE)
    .union(hyper_os::fs::FileRights::TRANSFER);

/// Fixed image and configuration namespace, independent of the live process
/// root. Keeping this capability across a root switch does not grant namespace
/// management or filesystem writes.
pub const ENVIRONMENT_DIRECTORY: StartupPurpose<DirectoryObject> = StartupPurpose::new(0x8004_0004);

/// Conventional current-directory authority delegated to command processes.
pub const WORKING_DIRECTORY: StartupPurpose<DirectoryObject> = StartupPurpose::new(0x8004_0001);

/// Library namespace which a process-construction service may attenuate into
/// child startup tables.
pub const CHILD_LIBRARY_DIRECTORY: StartupPurpose<DirectoryObject> =
    StartupPurpose::new(0x8004_0002);

/// Userspace deployment policy, independent of the kernel process contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoaderKind {
    Bootstrap,
    Dynamic,
}

macro_rules! loader_paths {
    ($arch:literal) => {
        const BOOTSTRAP_LOADER: &str = concat!("lib64/userspace-loader-hyper-", $arch);
        const DYNAMIC_LOADER: &str = concat!("lib64/ld-hyper-", $arch, ".so");
        const LIBRARIES: &str = concat!("lib64/", $arch, "-hyper-hyper");
    };
}
#[cfg(target_arch = "aarch64")]
loader_paths!("aarch64");
#[cfg(target_arch = "riscv64")]
loader_paths!("riscv64");
#[cfg(target_arch = "x86_64")]
loader_paths!("x86_64");

/// Capture a fixed, confined image environment. Both absolute and relative
/// paths through the returned capability stay within this directory, even if
/// another service later changes the live namespace root.
pub fn open_environment(
    root: &hyper_os::fs::Directory,
    path: &str,
) -> hyper_os::Result<hyper_os::fs::Directory> {
    use hyper_os::fs::DirectoryRights;
    let rights = DirectoryRights::READ
        .union(DirectoryRights::INSPECT)
        .union(DirectoryRights::EXECUTE)
        .union(DirectoryRights::PROTECT)
        .union(DirectoryRights::DUPLICATE)
        .union(DirectoryRights::TRANSFER);
    let directory = root.open_directory(path, rights)?;
    directory.scope_at(&directory, rights)
}

/// Opens exactly the selected environment's loader. Absence never selects a
/// different loader or changes the static/dynamic launch policy.
pub fn open_loader(
    environment: &hyper_os::fs::Directory,
    kind: LoaderKind,
) -> hyper_os::Result<hyper_os::fs::File> {
    use hyper_os::fs::FileRights;
    environment.open(
        match kind {
            LoaderKind::Bootstrap => BOOTSTRAP_LOADER,
            LoaderKind::Dynamic => DYNAMIC_LOADER,
        },
        FileRights::READ
            .union(FileRights::EXECUTE)
            .union(FileRights::DUPLICATE)
            .union(FileRights::TRANSFER),
    )
}

/// Opens the environment's library namespace, including the empty namespace
/// supplied to static recovery processes.
pub fn open_library_directory(
    environment: &hyper_os::fs::Directory,
) -> hyper_os::Result<hyper_os::fs::Directory> {
    use hyper_os::fs::DirectoryRights;
    environment.open_directory(
        LIBRARIES,
        DirectoryRights::READ
            .union(DirectoryRights::EXECUTE)
            .union(DirectoryRights::PROTECT)
            .union(DirectoryRights::DUPLICATE)
            .union(DirectoryRights::TRANSFER),
    )
}

const fn delegated(rights: Rights) -> Rights {
    rights.union(Rights::DUPLICATE).union(Rights::TRANSFER)
}

pub const ENVIRONMENT_DIRECTORY_CONTRACT: StartupContract = StartupContract::exact(
    ENVIRONMENT_DIRECTORY_NAME,
    ENVIRONMENT_DIRECTORY,
    Rights::READ
        .union(Rights::INSPECT)
        .union(Rights::EXECUTE)
        .union(Rights::PROTECT)
        .union(Rights::DUPLICATE)
        .union(Rights::TRANSFER),
);

pub const SHELL_ROOT_DIRECTORY_CONTRACT: StartupContract = StartupContract::exact(
    ROOT_DIRECTORY_NAME,
    startup::ROOT_DIRECTORY,
    Rights::READ
        .union(Rights::WRITE)
        .union(Rights::SET_ATTRIBUTES)
        .union(Rights::LOCK_FILE)
        .union(Rights::INSPECT)
        .union(Rights::EXECUTE)
        .union(Rights::PROTECT)
        .union(Rights::DUPLICATE)
        .union(Rights::TRANSFER),
);
pub const SHELL_TASK_FACTORY_CONTRACT: StartupContract = StartupContract::exact(
    TASK_FACTORY_NAME,
    startup::TASK_FACTORY,
    delegated(Rights::CREATE_PROCESS),
);
pub const SHELL_TASK_GROUP_CONTRACT: StartupContract = StartupContract::exact(
    TASK_GROUP_NAME,
    startup::TASK_GROUP,
    delegated(Rights::TASK_GROUP_ATTACH_PROCESS),
);
pub const SHELL_RESOURCE_DOMAIN_CONTRACT: StartupContract = StartupContract::exact(
    RESOURCE_DOMAIN_NAME,
    startup::RESOURCE_DOMAIN,
    delegated(Rights::RESOURCE_DOMAIN_SPONSOR),
);
pub const SHELL_TASK_INSPECTOR_CONTRACT: StartupContract = StartupContract::exact(
    TASK_INSPECTOR_NAME,
    startup::TASK_INSPECTOR,
    delegated(Rights::INSPECT),
);
pub const SHELL_OBJECT_INSPECTOR_CONTRACT: StartupContract = StartupContract::exact(
    OBJECT_INSPECTOR_NAME,
    startup::OBJECT_INSPECTOR,
    delegated(Rights::INSPECT.union(Rights::INSPECT_DETAILS)),
);
pub const SHELL_MEMORY_INSPECTOR_CONTRACT: StartupContract = StartupContract::exact(
    MEMORY_INSPECTOR_NAME,
    startup::MEMORY_INSPECTOR,
    delegated(Rights::INSPECT),
);
pub const SHELL_CPU_INSPECTOR_CONTRACT: StartupContract = StartupContract::exact(
    CPU_INSPECTOR_NAME,
    startup::CPU_INSPECTOR,
    delegated(Rights::INSPECT),
);
pub const CHILD_LOADER_CONTRACT: StartupContract = StartupContract::exact(
    CHILD_LOADER_NAME,
    CHILD_LOADER,
    delegated(Rights::READ.union(Rights::EXECUTE)),
);
pub const CHILD_LIBRARY_DIRECTORY_CONTRACT: StartupContract = StartupContract::exact(
    CHILD_LIBRARY_DIRECTORY_NAME,
    CHILD_LIBRARY_DIRECTORY,
    Rights::READ
        .union(Rights::EXECUTE)
        .union(Rights::PROTECT)
        .union(Rights::DUPLICATE)
        .union(Rights::TRANSFER),
);
pub const VM_MANAGER_TASK_FACTORY_CONTRACT: StartupContract = StartupContract::exact(
    TASK_FACTORY_NAME,
    startup::TASK_FACTORY,
    Rights::CREATE_PROCESS
        .union(Rights::CREATE_TASK_GROUP)
        .union(Rights::RETAIN_PROCESS),
);
pub const VM_MANAGER_RESOURCE_DOMAIN_CONTRACT: StartupContract = StartupContract::exact(
    RESOURCE_DOMAIN_NAME,
    startup::RESOURCE_DOMAIN,
    Rights::CREATE_RESOURCE_DOMAIN,
);

pub const SHELL_STARTUP_CONTRACTS: &[StartupContract] = &[
    ENVIRONMENT_DIRECTORY_CONTRACT,
    SHELL_ROOT_DIRECTORY_CONTRACT,
    SHELL_TASK_FACTORY_CONTRACT,
    SHELL_TASK_GROUP_CONTRACT,
    SHELL_RESOURCE_DOMAIN_CONTRACT,
    SHELL_TASK_INSPECTOR_CONTRACT,
    SHELL_OBJECT_INSPECTOR_CONTRACT,
    SHELL_MEMORY_INSPECTOR_CONTRACT,
    SHELL_CPU_INSPECTOR_CONTRACT,
    CHILD_LIBRARY_DIRECTORY_CONTRACT,
    CHILD_LOADER_CONTRACT,
    crate::filesystem::DELEGATED_CONTROL_CLIENT_CONTRACT,
];

pub const VM_MANAGER_STARTUP_CONTRACTS: &[StartupContract] = &[
    CHILD_LIBRARY_DIRECTORY_CONTRACT,
    CHILD_LOADER_CONTRACT,
    VM_MANAGER_TASK_FACTORY_CONTRACT,
    VM_MANAGER_RESOURCE_DOMAIN_CONTRACT,
];

/// Standard application authorities, independently attenuable in the manifest.
/// Existing shell/manager contracts retain their stronger, role-specific minima.
pub const APPLICATION_STARTUP_CONTRACTS: &[StartupContract] = &[
    ENVIRONMENT_DIRECTORY_CONTRACT,
    StartupContract::exact(ROOT_DIRECTORY_NAME, startup::ROOT_DIRECTORY, Rights::READ)
        .with_optional_rights(SHELL_ROOT_DIRECTORY_CONTRACT.allowed_rights()),
    StartupContract::exact(
        TASK_FACTORY_NAME,
        startup::TASK_FACTORY,
        Rights::CREATE_PROCESS,
    )
    // Deployment may explicitly delegate supervision to an application. This
    // ceiling grants nothing by itself; the shell's separate contract remains
    // limited to ordinary child creation.
    .with_optional_rights(
        SHELL_TASK_FACTORY_CONTRACT
            .allowed_rights()
            .union(Rights::CREATE_TASK_GROUP)
            .union(Rights::RETAIN_PROCESS),
    ),
    StartupContract::exact(
        TASK_GROUP_NAME,
        startup::TASK_GROUP,
        Rights::TASK_GROUP_ATTACH_PROCESS,
    )
    .with_optional_rights(SHELL_TASK_GROUP_CONTRACT.allowed_rights()),
    StartupContract::exact(
        RESOURCE_DOMAIN_NAME,
        startup::RESOURCE_DOMAIN,
        Rights::RESOURCE_DOMAIN_SPONSOR,
    )
    .with_optional_rights(SHELL_RESOURCE_DOMAIN_CONTRACT.allowed_rights()),
    StartupContract::exact(
        TASK_INSPECTOR_NAME,
        startup::TASK_INSPECTOR,
        Rights::INSPECT,
    )
    .with_optional_rights(SHELL_TASK_INSPECTOR_CONTRACT.allowed_rights()),
    StartupContract::exact(
        OBJECT_INSPECTOR_NAME,
        startup::OBJECT_INSPECTOR,
        Rights::INSPECT,
    )
    .with_optional_rights(SHELL_OBJECT_INSPECTOR_CONTRACT.allowed_rights()),
    StartupContract::exact(
        MEMORY_INSPECTOR_NAME,
        startup::MEMORY_INSPECTOR,
        Rights::INSPECT,
    )
    .with_optional_rights(SHELL_MEMORY_INSPECTOR_CONTRACT.allowed_rights()),
    StartupContract::exact(CPU_INSPECTOR_NAME, startup::CPU_INSPECTOR, Rights::INSPECT)
        .with_optional_rights(SHELL_CPU_INSPECTOR_CONTRACT.allowed_rights()),
    CHILD_LIBRARY_DIRECTORY_CONTRACT,
    CHILD_LOADER_CONTRACT,
];

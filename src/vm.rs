// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Startup and control-plane contracts for Native VM services.

use hyper_os::handle::{
    ByteChannelObject, CapabilityChannelObject, FileObject, Rights,
    VirtualMachineCreationLeaseObject,
};
use hyper_os::startup::StartupPurpose;

use crate::StartupContract;

pub const CREATION_LEASE_NAME: &str = "vm.creation-lease";
pub const CREATION_AUTHORITY_NAME: &str = "vm.creation-authority";
pub const IMAGE_NAME: &str = "vm.image";
pub const RUNTIME_IMAGE_NAME: &str = "vm.runtime-image";
pub const PROVISIONING_NAME: &str = "vm.provisioning";
pub const INSTANCE_CONTROL_NAME: &str = "vm.instance-control";
pub const CONSOLE_CONNECTION_NAME: &str = "vm.console-connection";
pub const MANAGER_CONNECTION_NAME: &str = "vm.manager-connection";
pub const CLIENT_CONTROL_NAME: &str = "vm.client-control";
pub const CLIENT_CAPABILITIES_NAME: &str = "vm.client-capabilities";

pub const CREATION_LEASE: StartupPurpose<VirtualMachineCreationLeaseObject> =
    StartupPurpose::new(0x8005_0001);
pub const IMAGE: StartupPurpose<FileObject> = StartupPurpose::new(0x8005_0002);
pub const RUNTIME_IMAGE: StartupPurpose<FileObject> = StartupPurpose::new(0x8005_0003);
pub const PROVISIONING: StartupPurpose<CapabilityChannelObject> = StartupPurpose::new(0x8005_0004);
pub const INSTANCE_CONTROL: StartupPurpose<ByteChannelObject> = StartupPurpose::new(0x8005_0005);
pub const CONSOLE_CONNECTION: StartupPurpose<CapabilityChannelObject> =
    StartupPurpose::new(0x8005_0006);
pub const MANAGER_CONNECTION: StartupPurpose<CapabilityChannelObject> =
    StartupPurpose::new(0x8005_0007);
pub const CLIENT_CONTROL: StartupPurpose<ByteChannelObject> = StartupPurpose::new(0x8005_0008);
pub const CLIENT_CAPABILITIES: StartupPurpose<CapabilityChannelObject> =
    StartupPurpose::new(0x8005_0009);

/// Rights retained while the manager duplicates a guest image into a runtime.
pub const MANAGED_IMAGE_RIGHTS: Rights = Rights::READ
    .union(Rights::DUPLICATE)
    .union(Rights::TRANSFER);
/// The fleet manager only reads its bootstrap configuration; it does not forward it.
pub const PROVISIONED_CONFIG_RIGHTS: Rights = Rights::READ;
/// The manager sends one configuration result, then closes this endpoint.
pub const PROVISIONED_RESULT_RIGHTS: Rights = Rights::WRITE;
/// Rights visible to one VM runtime for its instance-control endpoint.
pub const INSTANCE_CONTROL_RIGHTS: Rights = Rights::WAIT.union(Rights::READ).union(Rights::WRITE);
pub const RUNTIME_CONSOLE_CONNECTION_RIGHTS: Rights = Rights::WAIT.union(Rights::READ);
pub const CONSOLE_SESSION_RIGHTS: Rights = Rights::WAIT
    .union(Rights::READ)
    .union(Rights::WRITE)
    .union(Rights::TRANSFER);
pub const MANAGER_CONNECTION_RIGHTS: Rights = Rights::DUPLICATE
    .union(Rights::TRANSFER)
    .union(Rights::WAIT)
    .union(Rights::READ)
    .union(Rights::WRITE);

pub const MANAGER_RUNTIME_IMAGE_CONTRACT: StartupContract = StartupContract::exact(
    RUNTIME_IMAGE_NAME,
    RUNTIME_IMAGE,
    crate::process::EXECUTABLE_FILE_RIGHTS.as_rights(),
);
pub const MANAGER_PROVISIONING_CONTRACT: StartupContract = StartupContract::exact(
    PROVISIONING_NAME,
    PROVISIONING,
    Rights::WAIT.union(Rights::READ),
);
pub const MANAGER_CREATION_AUTHORITY_CONTRACT: StartupContract = StartupContract::exact(
    CREATION_AUTHORITY_NAME,
    hyper_os::startup::VIRTUAL_MACHINE_CREATION_AUTHORITY,
    Rights::DERIVE.union(Rights::CREATE_VIRTUAL_MACHINE),
)
.with_optional_rights(hyper_os::scheduling::RIGHTS);
pub const MANAGER_CONNECTION_CONTRACT: StartupContract = StartupContract::exact(
    MANAGER_CONNECTION_NAME,
    MANAGER_CONNECTION,
    Rights::WAIT.union(Rights::READ),
);

pub const MANAGER_STARTUP_CONTRACTS: &[StartupContract] = &[
    StartupContract::exact(
        crate::process::ROOT_DIRECTORY_NAME,
        hyper_os::startup::ROOT_DIRECTORY,
        Rights::READ
            .union(Rights::DUPLICATE)
            .union(Rights::TRANSFER),
    ),
    MANAGER_RUNTIME_IMAGE_CONTRACT,
    MANAGER_PROVISIONING_CONTRACT,
    MANAGER_CREATION_AUTHORITY_CONTRACT,
    MANAGER_CONNECTION_CONTRACT,
    crate::io::BROKER_CLIENT_CONTRACT,
];

pub const RUNTIME_IMAGE_CONTRACT: StartupContract =
    StartupContract::exact(IMAGE_NAME, IMAGE, Rights::READ);
pub const RUNTIME_CREATION_LEASE_CONTRACT: StartupContract = StartupContract::exact(
    CREATION_LEASE_NAME,
    CREATION_LEASE,
    Rights::CREATE_VIRTUAL_MACHINE.union(Rights::INSPECT),
)
.with_optional_rights(hyper_os::scheduling::RIGHTS);
pub const RUNTIME_INSTANCE_CONTROL_CONTRACT: StartupContract = StartupContract::exact(
    INSTANCE_CONTROL_NAME,
    INSTANCE_CONTROL,
    INSTANCE_CONTROL_RIGHTS,
);
pub const RUNTIME_CONSOLE_CONNECTION_CONTRACT: StartupContract = StartupContract::exact(
    CONSOLE_CONNECTION_NAME,
    CONSOLE_CONNECTION,
    RUNTIME_CONSOLE_CONNECTION_RIGHTS,
);

/// Complete startup vocabulary accepted by one per-VM runtime.
///
/// The connector receives authorized client byte channels from the manager.
pub const RUNTIME_STARTUP_CONTRACTS: &[StartupContract] = &[
    RUNTIME_IMAGE_CONTRACT,
    RUNTIME_CREATION_LEASE_CONTRACT,
    RUNTIME_INSTANCE_CONTROL_CONTRACT,
    RUNTIME_CONSOLE_CONNECTION_CONTRACT,
    crate::io::SESSION_CONTRACT,
];

pub const CLIENT_CONTROL_CONTRACT: StartupContract = StartupContract::exact(
    CLIENT_CONTROL_NAME,
    CLIENT_CONTROL,
    Rights::WAIT.union(Rights::READ).union(Rights::WRITE),
);
pub const CLIENT_CAPABILITIES_CONTRACT: StartupContract = StartupContract::exact(
    CLIENT_CAPABILITIES_NAME,
    CLIENT_CAPABILITIES,
    Rights::WAIT.union(Rights::READ),
);
pub const CLIENT_STARTUP_CONTRACTS: &[StartupContract] = &[StartupContract::exact(
    MANAGER_CONNECTION_NAME,
    MANAGER_CONNECTION,
    Rights::WAIT.union(Rights::WRITE),
)];
/// A shell or console manager may pass this client connector to another trusted
/// shell. Individual vmm processes receive dedicated channels instead.
pub const DELEGATED_CLIENT_CONNECTION_CONTRACT: StartupContract = StartupContract::exact(
    MANAGER_CONNECTION_NAME,
    MANAGER_CONNECTION,
    Rights::WAIT
        .union(Rights::WRITE)
        .union(Rights::DUPLICATE)
        .union(Rights::TRANSFER),
);
pub const VMM_STARTUP_CONTRACTS: &[StartupContract] =
    &[CLIENT_CONTROL_CONTRACT, CLIENT_CAPABILITIES_CONTRACT];

pub const MESSAGE_BYTES: usize = 8;
const MESSAGE_MAGIC: [u8; 3] = *b"HVM";
const MESSAGE_VERSION: u8 = 1;
const KIND_PROVISION_REQUEST: u8 = 1;
const KIND_INSTANCE_COMMAND: u8 = 2;
const KIND_INSTANCE_STATUS: u8 = 3;
const KIND_MANAGER_CONNECTION: u8 = 5;
const KIND_PROVISION_RESULT: u8 = 6;
const KIND_FLEET_CAPABILITY: u8 = 8;

/// Completes initial provisioning with a configuration or an explicit
/// unavailable state. Closing the channel is never a successful request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProvisionRequest {
    /// Installs definitions as one batch. Carries the read-only configuration
    /// file and a write-only, one-shot result endpoint, in that order.
    ConfigureFleet,
    /// Ends initial provisioning without a configuration or broker discovery.
    /// Carries no capabilities; the receive rendezvous completes this request.
    ConfigurationUnavailable,
}

impl ProvisionRequest {
    #[must_use]
    pub const fn encode(self) -> [u8; MESSAGE_BYTES] {
        match self {
            Self::ConfigureFleet => encode_message(KIND_PROVISION_REQUEST, 1, 0),
            Self::ConfigurationUnavailable => encode_message(KIND_PROVISION_REQUEST, 2, 0),
        }
    }

    #[must_use]
    pub fn decode(message: &[u8]) -> Option<Self> {
        let (value, detail) = decode_message(message, KIND_PROVISION_REQUEST)?;
        match (value, detail) {
            (1, 0) => Some(Self::ConfigureFleet),
            (2, 0) => Some(Self::ConfigurationUnavailable),
            _ => None,
        }
    }

    #[must_use]
    pub const fn capability_count(self) -> usize {
        match self {
            Self::ConfigureFleet => 2,
            Self::ConfigurationUnavailable => 0,
        }
    }
}

/// Configuration admission result, independent of guest startup or lifetime.
/// Sent before autostart begins; an empty fleet is also successfully configured.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProvisionResult {
    Configured,
    Rejected,
}

impl ProvisionResult {
    #[must_use]
    pub const fn encode(self) -> [u8; MESSAGE_BYTES] {
        let value = match self {
            Self::Configured => 1,
            Self::Rejected => 2,
        };
        encode_message(KIND_PROVISION_RESULT, value, 0)
    }

    #[must_use]
    pub fn decode(message: &[u8]) -> Option<Self> {
        match decode_message(message, KIND_PROVISION_RESULT)? {
            (1, 0) => Some(Self::Configured),
            (2, 0) => Some(Self::Rejected),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ManagerConnectionRequest;

impl ManagerConnectionRequest {
    pub const CAPABILITY_COUNT: usize = 2;

    #[must_use]
    pub const fn encode(self) -> [u8; MESSAGE_BYTES] {
        encode_message(KIND_MANAGER_CONNECTION, 1, 0)
    }

    #[must_use]
    pub fn decode(message: &[u8]) -> Option<Self> {
        matches!(
            decode_message(message, KIND_MANAGER_CONNECTION),
            Some((1, 0))
        )
        .then_some(Self)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConsoleCapability;

impl ConsoleCapability {
    #[must_use]
    pub const fn encode(self) -> [u8; MESSAGE_BYTES] {
        encode_message(KIND_FLEET_CAPABILITY, 1, 0)
    }

    #[must_use]
    pub fn decode(message: &[u8]) -> Option<Self> {
        matches!(decode_message(message, KIND_FLEET_CAPABILITY), Some((1, 0))).then_some(Self)
    }
}

/// Bounded, request-correlated inspection separate from lifecycle status messages.
pub const OBSERVATION_BYTES: usize = 40;
const KIND_OBSERVATION_REQUEST: u8 = 9;
const KIND_OBSERVATION: u8 = 10;
const KIND_VCPU_CONTROL_REQUEST: u8 = 11;
const KIND_VCPU_CONTROL_REPLY: u8 = 12;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ObservationRequest(pub u64);

impl ObservationRequest {
    #[must_use]
    pub fn encode(self) -> [u8; 16] {
        let mut bytes = [0; 16];
        bytes[..8].copy_from_slice(&encode_message(KIND_OBSERVATION_REQUEST, 1, 0));
        bytes[8..].copy_from_slice(&self.0.to_le_bytes());
        bytes
    }

    #[must_use]
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != 16
            || decode_message(&bytes[..8], KIND_OBSERVATION_REQUEST) != Some((1, 0))
        {
            return None;
        }
        Some(Self(u64::from_le_bytes(bytes[8..16].try_into().ok()?)))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Observation {
    pub request: ObservationRequest,
    pub vcpus: u32,
    pub capacity_bytes: u64,
    pub resident_bytes: Option<u64>,
}

impl Observation {
    #[must_use]
    pub fn encode(self) -> [u8; OBSERVATION_BYTES] {
        let mut bytes = [0; OBSERVATION_BYTES];
        bytes[..8].copy_from_slice(&encode_message(KIND_OBSERVATION, 1, 0));
        bytes[8..16].copy_from_slice(&self.request.0.to_le_bytes());
        bytes[16..20].copy_from_slice(&self.vcpus.to_le_bytes());
        bytes[24..32].copy_from_slice(&self.capacity_bytes.to_le_bytes());
        bytes[32..40].copy_from_slice(&self.resident_bytes.unwrap_or(u64::MAX).to_le_bytes());
        bytes
    }

    #[must_use]
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != OBSERVATION_BYTES
            || decode_message(&bytes[..8], KIND_OBSERVATION) != Some((1, 0))
            || bytes[20..24] != [0; 4]
        {
            return None;
        }
        let resident = u64::from_le_bytes(bytes[32..40].try_into().ok()?);
        let capacity = u64::from_le_bytes(bytes[24..32].try_into().ok()?);
        let vcpus = u32::from_le_bytes(bytes[16..20].try_into().ok()?);
        if vcpus == 0 {
            return None;
        }
        Some(Self {
            request: ObservationRequest(u64::from_le_bytes(bytes[8..16].try_into().ok()?)),
            vcpus,
            capacity_bytes: capacity,
            resident_bytes: (resident != u64::MAX).then_some(resident),
        })
    }
}

/// Bounded control storage, also large enough for observations and lifecycle records.
pub const VCPU_AFFINITY_WORDS: usize = hyper_os::vm::VCPU_AFFINITY_MAX_WORDS;
pub const CONTROL_BYTES: usize = 24 + VCPU_AFFINITY_WORDS * 8;

/// Per-vCPU inspection and allowed-CPU policy; migration is a scheduler effect.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VcpuControlRequest {
    pub sequence: u64,
    pub vcpu: u32,
    /// None inspects placement without changing policy.
    pub affinity: Option<[u64; VCPU_AFFINITY_WORDS]>,
}
impl VcpuControlRequest {
    #[must_use]
    pub fn encode(self) -> [u8; CONTROL_BYTES] {
        let mut bytes = [0; CONTROL_BYTES];
        bytes[..8].copy_from_slice(&encode_message(
            KIND_VCPU_CONTROL_REQUEST,
            if self.affinity.is_some() { 2 } else { 1 },
            0,
        ));
        bytes[8..16].copy_from_slice(&self.sequence.to_le_bytes());
        bytes[16..20].copy_from_slice(&self.vcpu.to_le_bytes());
        if let Some(words) = self.affinity {
            for (chunk, word) in bytes[24..].chunks_exact_mut(8).zip(words) {
                chunk.copy_from_slice(&word.to_le_bytes());
            }
        }
        bytes
    }
    #[must_use]
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != CONTROL_BYTES || bytes[20..24] != [0; 4] {
            return None;
        }
        let (kind, detail) = decode_message(&bytes[..8], KIND_VCPU_CONTROL_REQUEST)?;
        if !matches!(kind, 1 | 2) || detail != 0 {
            return None;
        }
        let mut words = [0; VCPU_AFFINITY_WORDS];
        for (word, chunk) in words.iter_mut().zip(bytes[24..].chunks_exact(8)) {
            *word = u64::from_le_bytes(chunk.try_into().ok()?);
        }
        if kind == 1 && words.iter().any(|word| *word != 0) {
            return None;
        }
        Some(Self {
            sequence: u64::from_le_bytes(bytes[8..16].try_into().ok()?),
            vcpu: u32::from_le_bytes(bytes[16..20].try_into().ok()?),
            affinity: (kind == 2).then_some(words),
        })
    }
}
/// An acknowledgement of policy acceptance plus a best-effort placement snapshot.
/// Success does not promise that a required migration has already completed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VcpuControlReply {
    pub sequence: u64,
    pub vcpu: u32,
    pub host_cpu: Option<u32>,
    pub pending_host_cpu: Option<u32>,
    pub status: hyper_os::Status,
}
impl VcpuControlReply {
    #[must_use]
    pub fn encode(self) -> [u8; OBSERVATION_BYTES] {
        let mut bytes = [0; OBSERVATION_BYTES];
        bytes[..8].copy_from_slice(&encode_message(KIND_VCPU_CONTROL_REPLY, 1, 0));
        bytes[8..16].copy_from_slice(&self.sequence.to_le_bytes());
        bytes[16..20].copy_from_slice(&self.vcpu.to_le_bytes());
        bytes[20..24].copy_from_slice(&self.host_cpu.unwrap_or(u32::MAX).to_le_bytes());
        bytes[24..28].copy_from_slice(&self.pending_host_cpu.unwrap_or(u32::MAX).to_le_bytes());
        bytes[32..40].copy_from_slice(&self.status.as_raw().to_le_bytes());
        bytes
    }
    #[must_use]
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != OBSERVATION_BYTES
            || bytes[28..32] != [0; 4]
            || decode_message(&bytes[..8], KIND_VCPU_CONTROL_REPLY) != Some((1, 0))
        {
            return None;
        }
        let cpu = |range: core::ops::Range<usize>| {
            let raw = u32::from_le_bytes(bytes[range].try_into().ok()?);
            Some((raw != u32::MAX).then_some(raw))
        };
        Some(Self {
            sequence: u64::from_le_bytes(bytes[8..16].try_into().ok()?),
            vcpu: u32::from_le_bytes(bytes[16..20].try_into().ok()?),
            host_cpu: cpu(20..24)?,
            pending_host_cpu: cpu(24..28)?,
            status: hyper_os::Status::from_raw(i64::from_le_bytes(bytes[32..40].try_into().ok()?)),
        })
    }
}

/// Cooperative command sent from a fleet manager to one VM runtime.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum InstanceCommand {
    Stop = 1,
    AttachConsole = 2,
}

impl InstanceCommand {
    #[must_use]
    pub const fn encode(self) -> [u8; MESSAGE_BYTES] {
        encode_message(KIND_INSTANCE_COMMAND, self as u8, 0)
    }

    #[must_use]
    pub fn decode(message: &[u8]) -> Option<Self> {
        let (value, detail) = decode_message(message, KIND_INSTANCE_COMMAND)?;
        match (value, detail) {
            (1, 0) => Some(Self::Stop),
            (2, 0) => Some(Self::AttachConsole),
            _ => None,
        }
    }
}

/// Failure classification retained across the runtime/manager boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum InstanceFailure {
    Runtime = 1,
    InvalidImage = 2,
    GuestMemoryFault = 3,
    GuestMmio = 4,
    GuestSynchronous = 5,
    UnexpectedAdministrativeStop = 6,
    InvalidControlProtocol = 7,
    MissingTerminalStatus = 8,
    UnsupportedConfiguration = 9,
}

/// One manager action produced by an idempotent stop-state transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StopAction {
    None,
    SendCooperative,
    ForceProcess,
}

/// Stop escalation state retained independently of transport readiness.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InstanceStopState {
    phase: StopPhase,
    stop_requested: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StopPhase {
    Running,
    CooperativeRequested,
    Forced,
}

impl InstanceStopState {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            phase: StopPhase::Running,
            stop_requested: false,
        }
    }

    /// Explicit stop policy, distinct from cleanup escalation after a guest
    /// terminal status. An expired cleanup timer must not cancel a reboot that
    /// already completed successfully before the manager observed process exit.
    #[must_use]
    pub const fn has_stop_request(self) -> bool {
        self.stop_requested
    }

    /// Begins graceful shutdown exactly once.
    pub fn request_cooperative(&mut self) -> StopAction {
        self.stop_requested = true;
        match self.phase {
            StopPhase::Running => {
                self.phase = StopPhase::CooperativeRequested;
                StopAction::SendCooperative
            }
            StopPhase::CooperativeRequested | StopPhase::Forced => StopAction::None,
        }
    }

    /// Stops for explicit policy such as owner loss or protocol corruption.
    pub fn request_forced(&mut self) -> StopAction {
        self.stop_requested = true;
        self.escalate()
    }

    fn escalate(&mut self) -> StopAction {
        match self.phase {
            StopPhase::Forced => StopAction::None,
            StopPhase::Running | StopPhase::CooperativeRequested => {
                self.phase = StopPhase::Forced;
                StopAction::ForceProcess
            }
        }
    }

    /// Escalates cleanup after its externally supplied grace deadline expires,
    /// preserving whether an explicit stop was requested.
    ///
    /// The protocol deliberately does not synthesize time. A manager must call
    /// this transition only from a real monotonic-deadline observation.
    pub fn grace_period_expired(&mut self) -> StopAction {
        self.escalate()
    }
}

impl Default for InstanceStopState {
    fn default() -> Self {
        Self::new()
    }
}

impl InstanceFailure {
    const fn from_wire(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::Runtime),
            2 => Some(Self::InvalidImage),
            3 => Some(Self::GuestMemoryFault),
            4 => Some(Self::GuestMmio),
            5 => Some(Self::GuestSynchronous),
            6 => Some(Self::UnexpectedAdministrativeStop),
            7 => Some(Self::InvalidControlProtocol),
            8 => Some(Self::MissingTerminalStatus),
            9 => Some(Self::UnsupportedConfiguration),
            _ => None,
        }
    }
}

/// Coarse-grained construction and execution progress from one VM runtime.
///
/// This low-frequency control protocol never carries guest data. Status
/// validation is monotonic, so duplicate, skipped, or post-terminal messages
/// are rejected before they can alter fleet policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InstanceStatus {
    ImageValidated,
    MemoryPrepared,
    Installed,
    Running,
    Stopped,
    /// The retired instance requests a fresh VM from its manager.
    RebootRequested,
    Failed(InstanceFailure),
}

impl InstanceStatus {
    #[must_use]
    pub const fn encode(self) -> [u8; MESSAGE_BYTES] {
        match self {
            Self::ImageValidated => encode_message(KIND_INSTANCE_STATUS, 1, 0),
            Self::MemoryPrepared => encode_message(KIND_INSTANCE_STATUS, 2, 0),
            Self::Installed => encode_message(KIND_INSTANCE_STATUS, 3, 0),
            Self::Running => encode_message(KIND_INSTANCE_STATUS, 4, 0),
            Self::Stopped => encode_message(KIND_INSTANCE_STATUS, 5, 0),
            Self::RebootRequested => encode_message(KIND_INSTANCE_STATUS, 7, 0),
            Self::Failed(reason) => encode_message(KIND_INSTANCE_STATUS, 6, reason as u8),
        }
    }

    #[must_use]
    pub fn decode(message: &[u8]) -> Option<Self> {
        let (value, detail) = decode_message(message, KIND_INSTANCE_STATUS)?;
        match (value, detail) {
            (1, 0) => Some(Self::ImageValidated),
            (2, 0) => Some(Self::MemoryPrepared),
            (3, 0) => Some(Self::Installed),
            (4, 0) => Some(Self::Running),
            (5, 0) => Some(Self::Stopped),
            (7, 0) => Some(Self::RebootRequested),
            (6, reason) => Some(Self::Failed(InstanceFailure::from_wire(reason)?)),
            _ => None,
        }
    }

    /// Validates the lifecycle sequence emitted by one runtime.
    #[must_use]
    pub const fn follows(self, previous: Option<Self>) -> bool {
        matches!(
            (previous, self),
            (None, Self::ImageValidated)
                | (None, Self::Failed(_))
                | (Some(Self::ImageValidated), Self::MemoryPrepared)
                | (Some(Self::ImageValidated), Self::Failed(_))
                | (Some(Self::MemoryPrepared), Self::Installed)
                | (Some(Self::MemoryPrepared), Self::Failed(_))
                | (Some(Self::Installed), Self::Running)
                | (Some(Self::Installed), Self::Failed(_))
                | (Some(Self::Running), Self::Stopped)
                | (Some(Self::Running), Self::RebootRequested)
                | (Some(Self::Running), Self::Failed(_))
        )
    }

    #[must_use]
    pub const fn terminal(self) -> Option<InstanceEvent> {
        match self {
            Self::Stopped | Self::RebootRequested => Some(InstanceEvent::Stopped),
            Self::Failed(reason) => Some(InstanceEvent::Failed(reason)),
            Self::ImageValidated | Self::MemoryPrepared | Self::Installed | Self::Running => None,
        }
    }
}

/// Terminal instance outcome derived by the manager from runtime status and exit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InstanceEvent {
    Stopped,
    Failed(InstanceFailure),
}

/// Pure reducer used by the manager to validate one runtime status stream.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InstanceTracker {
    last: Option<InstanceStatus>,
    protocol_failed: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidStatusTransition;

impl InstanceTracker {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            last: None,
            protocol_failed: false,
        }
    }

    /// Records one decoded message. Invalid ordering permanently poisons the
    /// stream so a later plausible terminal record cannot hide corruption.
    pub fn observe(&mut self, status: InstanceStatus) -> Result<(), InvalidStatusTransition> {
        if self.protocol_failed || !status.follows(self.last) {
            self.protocol_failed = true;
            return Err(InvalidStatusTransition);
        }
        self.last = Some(status);
        Ok(())
    }

    /// Permanently marks a malformed wire record or transport transition.
    pub fn reject_protocol(&mut self) {
        self.protocol_failed = true;
    }

    /// Reports whether a valid terminal record has already been observed.
    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        !self.protocol_failed
            && matches!(
                self.last,
                Some(
                    InstanceStatus::Stopped
                        | InstanceStatus::RebootRequested
                        | InstanceStatus::Failed(_)
                )
            )
    }

    #[must_use]
    pub const fn last_status(&self) -> Option<InstanceStatus> {
        self.last
    }

    /// A validated guest reset survives delayed observation of successful exit,
    /// but never an explicit stop, invalid protocol, or an unsuccessful exit.
    #[must_use]
    pub const fn should_restart(self, process_succeeded: bool, stop: InstanceStopState) -> bool {
        process_succeeded
            && self.is_terminal()
            && matches!(self.last, Some(InstanceStatus::RebootRequested))
            && !stop.has_stop_request()
    }

    /// Combines the validated terminal record with process-exit status.
    #[must_use]
    pub const fn finish(self, process_succeeded: bool) -> InstanceEvent {
        if self.protocol_failed {
            return InstanceEvent::Failed(InstanceFailure::InvalidControlProtocol);
        }
        match (process_succeeded, self.last) {
            (true, Some(status)) => match status.terminal() {
                Some(event) => event,
                None => InstanceEvent::Failed(InstanceFailure::MissingTerminalStatus),
            },
            (true, None) => InstanceEvent::Failed(InstanceFailure::MissingTerminalStatus),
            (false, Some(InstanceStatus::Failed(reason))) => InstanceEvent::Failed(reason),
            (false, _) => InstanceEvent::Failed(InstanceFailure::Runtime),
        }
    }
}

impl Default for InstanceTracker {
    fn default() -> Self {
        Self::new()
    }
}

const fn encode_message(kind: u8, value: u8, detail: u8) -> [u8; MESSAGE_BYTES] {
    [
        MESSAGE_MAGIC[0],
        MESSAGE_MAGIC[1],
        MESSAGE_MAGIC[2],
        MESSAGE_VERSION,
        kind,
        value,
        detail,
        0,
    ]
}

fn decode_message(message: &[u8], expected_kind: u8) -> Option<(u8, u8)> {
    let bytes: &[u8; MESSAGE_BYTES] = message.try_into().ok()?;
    if bytes[..3] != MESSAGE_MAGIC
        || bytes[3] != MESSAGE_VERSION
        || bytes[4] != expected_kind
        || bytes[7] != 0
    {
        return None;
    }
    Some((bytes[5], bytes[6]))
}

#[cfg(test)]
mod tests {
    use super::{
        INSTANCE_CONTROL_RIGHTS, InstanceCommand, InstanceEvent, InstanceFailure, InstanceStatus,
        InstanceStopState, InstanceTracker, InvalidStatusTransition, MANAGED_IMAGE_RIGHTS,
        MANAGER_PROVISIONING_CONTRACT, ManagerConnectionRequest, PROVISIONED_CONFIG_RIGHTS,
        PROVISIONED_RESULT_RIGHTS, ProvisionRequest, ProvisionResult,
        RUNTIME_CONSOLE_CONNECTION_CONTRACT, RUNTIME_CREATION_LEASE_CONTRACT,
        RUNTIME_IMAGE_CONTRACT, RUNTIME_INSTANCE_CONTROL_CONTRACT, RUNTIME_STARTUP_CONTRACTS,
        StopAction,
    };
    use hyper_os::handle::Rights;

    #[test]
    fn provision_results_round_trip_independently_of_instance_messages() {
        for result in [ProvisionResult::Configured, ProvisionResult::Rejected] {
            let message = result.encode();
            assert_eq!(ProvisionResult::decode(&message), Some(result));
            assert_eq!(ProvisionRequest::decode(&message), None);
            assert_eq!(InstanceCommand::decode(&message), None);
            assert_eq!(InstanceStatus::decode(&message), None);
            assert_eq!(ManagerConnectionRequest::decode(&message), None);
        }
        for message in [
            ProvisionRequest::ConfigureFleet.encode(),
            InstanceCommand::Stop.encode(),
            InstanceStatus::Stopped.encode(),
            InstanceStatus::Failed(InstanceFailure::Runtime).encode(),
            ManagerConnectionRequest.encode(),
            // Removed boot-time instance notifications are not admission results.
            [b'H', b'V', b'M', 1, 4, 1, 0, 0],
            [b'H', b'V', b'M', 1, 4, 2, 1, 0],
            [b'H', b'V', b'M', 1, 4, 3, 0, 0],
        ] {
            assert_eq!(ProvisionResult::decode(&message), None);
        }
    }

    #[test]
    fn provision_results_reject_malformed_envelopes_and_reserved_values() {
        for result in [ProvisionResult::Configured, ProvisionResult::Rejected] {
            let message = result.encode();
            assert_eq!(ProvisionResult::decode(&message[..7]), None);
            let mut extended = [0; 9];
            extended[..8].copy_from_slice(&message);
            assert_eq!(ProvisionResult::decode(&extended), None);
            for (index, value) in [
                (0, 0),
                (1, 0),
                (2, 0),
                (3, 0),
                (3, 2),
                (4, 0),
                (5, 0),
                (5, 3),
                (5, u8::MAX),
                (6, 1),
                (7, 1),
            ] {
                let mut malformed = message;
                malformed[index] = value;
                assert_eq!(ProvisionResult::decode(&malformed), None);
            }
        }
    }

    #[test]
    fn reboot_is_terminal_only_after_running_and_requires_successful_exit() {
        let mut tracker = InstanceTracker::new();
        assert!(!InstanceStatus::RebootRequested.follows(None));
        assert_eq!(
            InstanceStatus::decode(&InstanceStatus::RebootRequested.encode()),
            Some(InstanceStatus::RebootRequested)
        );
        for status in [
            InstanceStatus::ImageValidated,
            InstanceStatus::MemoryPrepared,
            InstanceStatus::Installed,
            InstanceStatus::Running,
            InstanceStatus::RebootRequested,
        ] {
            assert!(tracker.observe(status).is_ok());
        }
        assert!(tracker.is_terminal());
        assert_eq!(tracker.finish(true), InstanceEvent::Stopped);
        assert_eq!(
            tracker.finish(false),
            InstanceEvent::Failed(InstanceFailure::Runtime)
        );
        assert!(tracker.observe(InstanceStatus::Running).is_err());
        assert!(!tracker.is_terminal());
    }

    #[test]
    fn provisioned_capabilities_grant_only_configuration_and_result_access() {
        assert!(MANAGED_IMAGE_RIGHTS.contains(Rights::TRANSFER));
        assert_eq!(PROVISIONED_CONFIG_RIGHTS, Rights::READ);
        assert_eq!(PROVISIONED_RESULT_RIGHTS, Rights::WRITE);
        assert_eq!(
            MANAGER_PROVISIONING_CONTRACT.required_rights(),
            Rights::READ.union(Rights::WAIT)
        );
        assert_eq!(
            MANAGER_PROVISIONING_CONTRACT.allowed_rights(),
            MANAGER_PROVISIONING_CONTRACT.required_rights()
        );
        assert!(!INSTANCE_CONTROL_RIGHTS.contains(Rights::TRANSFER));
    }

    #[test]
    fn runtime_startup_contract_is_complete_exact_and_unambiguous() {
        assert_eq!(
            RUNTIME_STARTUP_CONTRACTS,
            &[
                RUNTIME_IMAGE_CONTRACT,
                RUNTIME_CREATION_LEASE_CONTRACT,
                RUNTIME_INSTANCE_CONTROL_CONTRACT,
                RUNTIME_CONSOLE_CONNECTION_CONTRACT,
                crate::io::SESSION_CONTRACT,
            ]
        );
        for (index, contract) in RUNTIME_STARTUP_CONTRACTS.iter().enumerate() {
            if *contract == RUNTIME_CREATION_LEASE_CONTRACT {
                assert_eq!(
                    contract
                        .required_rights()
                        .union(hyper_os::scheduling::RIGHTS),
                    contract.allowed_rights()
                );
            } else {
                assert_eq!(contract.required_rights(), contract.allowed_rights());
            }
            for other in RUNTIME_STARTUP_CONTRACTS.iter().skip(index + 1) {
                assert_ne!(contract.name(), other.name());
                assert_ne!(contract.purpose(), other.purpose());
            }
        }
    }

    #[test]
    fn runtime_lease_can_inspect_platform_then_create_without_forwarding_authority() {
        let rights = RUNTIME_CREATION_LEASE_CONTRACT.required_rights();
        assert!(rights.contains(Rights::INSPECT));
        assert!(rights.contains(Rights::CREATE_VIRTUAL_MACHINE));
        assert!(!rights.contains(Rights::TRANSFER));
        assert!(!rights.contains(Rights::DUPLICATE));
        assert_eq!(
            rights.union(hyper_os::scheduling::RIGHTS),
            RUNTIME_CREATION_LEASE_CONTRACT.allowed_rights()
        );
        assert!(!rights.contains(hyper_os::scheduling::RIGHTS));
    }

    #[test]
    fn wire_values_are_explicit_and_stable() {
        assert_eq!(
            ProvisionRequest::ConfigureFleet.encode(),
            [b'H', b'V', b'M', 1, 1, 1, 0, 0]
        );
        assert_eq!(
            InstanceCommand::Stop.encode(),
            [b'H', b'V', b'M', 1, 2, 1, 0, 0]
        );
        assert_eq!(
            InstanceStatus::Running.encode(),
            [b'H', b'V', b'M', 1, 3, 4, 0, 0]
        );
        assert_eq!(
            InstanceStatus::Failed(InstanceFailure::GuestMemoryFault).encode(),
            [b'H', b'V', b'M', 1, 3, 6, 3, 0]
        );
        assert_eq!(
            ProvisionResult::Configured.encode(),
            [b'H', b'V', b'M', 1, 6, 1, 0, 0]
        );
        assert_eq!(
            ProvisionResult::Rejected.encode(),
            [b'H', b'V', b'M', 1, 6, 2, 0, 0]
        );
    }

    #[test]
    fn message_kinds_cannot_be_confused() {
        let command = ProvisionRequest::ConfigureFleet.encode();
        assert_eq!(
            ProvisionRequest::decode(&command),
            Some(ProvisionRequest::ConfigureFleet)
        );
        assert_eq!(InstanceCommand::decode(&command), None);
        assert_eq!(InstanceStatus::decode(&command), None);
        assert_eq!(ProvisionResult::decode(&command), None);
    }

    #[test]
    fn manager_connection_round_trips() {
        assert_eq!(
            ManagerConnectionRequest::decode(&ManagerConnectionRequest.encode()),
            Some(ManagerConnectionRequest)
        );
    }

    #[test]
    fn lifecycle_reducer_accepts_only_complete_monotonic_paths() {
        let mut previous = None;
        for status in [
            InstanceStatus::ImageValidated,
            InstanceStatus::MemoryPrepared,
            InstanceStatus::Installed,
            InstanceStatus::Running,
            InstanceStatus::Stopped,
        ] {
            assert!(status.follows(previous));
            previous = Some(status);
        }
        assert_eq!(
            previous.and_then(InstanceStatus::terminal),
            Some(InstanceEvent::Stopped)
        );
        assert!(!InstanceStatus::Running.follows(Some(InstanceStatus::ImageValidated)));
        assert!(!InstanceStatus::Stopped.follows(Some(InstanceStatus::Stopped)));
    }

    #[test]
    fn failure_is_terminal_at_every_nonterminal_stage() {
        for previous in [
            None,
            Some(InstanceStatus::ImageValidated),
            Some(InstanceStatus::MemoryPrepared),
            Some(InstanceStatus::Installed),
            Some(InstanceStatus::Running),
        ] {
            let failed = InstanceStatus::Failed(InstanceFailure::Runtime);
            assert!(failed.follows(previous));
            assert_eq!(
                failed.terminal(),
                Some(InstanceEvent::Failed(InstanceFailure::Runtime))
            );
        }
    }

    #[test]
    fn malformed_and_reserved_messages_are_rejected() {
        let mut message = InstanceStatus::Running.encode();
        message[7] = 1;
        assert_eq!(InstanceStatus::decode(&message), None);
        assert_eq!(InstanceStatus::decode(&message[..7]), None);
    }

    #[test]
    fn fleet_configuration_requires_both_capabilities_atomically() {
        let request = ProvisionRequest::ConfigureFleet;
        assert_eq!(ProvisionRequest::decode(&request.encode()), Some(request));
        assert_eq!(request.capability_count(), 2);
    }

    #[test]
    fn unavailable_configuration_is_an_explicit_capability_free_terminal_request() {
        let request = ProvisionRequest::ConfigurationUnavailable;
        assert_eq!(request.encode(), [b'H', b'V', b'M', 1, 1, 2, 0, 0]);
        assert_eq!(ProvisionRequest::decode(&request.encode()), Some(request));
        assert_eq!(request.capability_count(), 0);
        assert_eq!(ProvisionResult::decode(&request.encode()), None);
        assert_eq!(ProvisionRequest::decode(&[]), None);
        for (index, value) in [(4, 6), (5, 3), (6, 1), (7, 1)] {
            let mut invalid = request.encode();
            invalid[index] = value;
            assert_eq!(ProvisionRequest::decode(&invalid), None);
        }
    }

    #[test]
    fn stop_escalation_is_idempotent_and_peer_close_can_force() {
        let mut state = InstanceStopState::new();
        assert_eq!(state.request_cooperative(), StopAction::SendCooperative);
        assert_eq!(state.request_cooperative(), StopAction::None);
        assert_eq!(state.grace_period_expired(), StopAction::ForceProcess);
        assert_eq!(state.request_forced(), StopAction::None);

        let mut owner_lost = InstanceStopState::new();
        assert_eq!(owner_lost.request_forced(), StopAction::ForceProcess);
        assert_eq!(owner_lost.request_cooperative(), StopAction::None);
    }

    fn reboot_tracker() -> InstanceTracker {
        let mut tracker = InstanceTracker::new();
        for status in [
            InstanceStatus::ImageValidated,
            InstanceStatus::MemoryPrepared,
            InstanceStatus::Installed,
            InstanceStatus::Running,
            InstanceStatus::RebootRequested,
        ] {
            assert_eq!(tracker.observe(status), Ok(()));
        }
        tracker
    }

    #[test]
    fn delayed_clean_exit_after_cleanup_timeout_still_reboots() {
        let tracker = reboot_tracker();
        let mut stop = InstanceStopState::new();
        assert!(tracker.should_restart(true, stop));
        // Exit completed first, but the manager observes its elapsed cleanup
        // deadline before consuming the process-termination notification.
        assert_eq!(stop.grace_period_expired(), StopAction::ForceProcess);
        assert_eq!(stop.grace_period_expired(), StopAction::None);
        assert!(!stop.has_stop_request());
        assert!(tracker.should_restart(true, stop));
        // A real timeout kill is still an unsuccessful process exit.
        assert!(!tracker.should_restart(false, stop));
    }

    #[test]
    fn explicit_stop_wins_before_or_after_cleanup_escalation() {
        let tracker = reboot_tracker();
        for force in [false, true] {
            for timeout_first in [false, true] {
                let mut stop = InstanceStopState::new();
                if timeout_first {
                    assert_eq!(stop.grace_period_expired(), StopAction::ForceProcess);
                }
                if force {
                    let _ = stop.request_forced();
                } else {
                    let _ = stop.request_cooperative();
                }
                let _ = stop.grace_period_expired();
                assert!(stop.has_stop_request());
                assert!(!tracker.should_restart(true, stop));
            }
        }
    }

    #[test]
    fn cleanup_timeout_cannot_authorize_invalid_reboot() {
        let mut stop = InstanceStopState::new();
        let _ = stop.grace_period_expired();
        assert!(!InstanceTracker::new().should_restart(true, stop));
        let mut tracker = reboot_tracker();
        tracker.reject_protocol();
        assert!(!tracker.should_restart(true, stop));
    }

    #[test]
    fn runtime_finalization_cannot_hide_missing_or_invalid_status() {
        let mut clean = InstanceTracker::new();
        for status in [
            InstanceStatus::ImageValidated,
            InstanceStatus::MemoryPrepared,
            InstanceStatus::Installed,
            InstanceStatus::Running,
            InstanceStatus::Stopped,
        ] {
            assert_eq!(clean.observe(status), Ok(()));
        }
        assert!(clean.is_terminal());
        assert_eq!(clean.finish(true), InstanceEvent::Stopped);

        let missing = InstanceTracker::new();
        assert!(!missing.is_terminal());
        assert_eq!(
            missing.finish(true),
            InstanceEvent::Failed(InstanceFailure::MissingTerminalStatus)
        );

        let mut invalid = InstanceTracker::new();
        assert_eq!(
            invalid.observe(InstanceStatus::Running),
            Err(InvalidStatusTransition)
        );
        assert_eq!(
            invalid.finish(true),
            InstanceEvent::Failed(InstanceFailure::InvalidControlProtocol)
        );
    }

    #[test]
    fn runtime_failure_reason_survives_process_failure() {
        let mut tracker = InstanceTracker::new();
        assert_eq!(
            tracker.observe(InstanceStatus::Failed(InstanceFailure::InvalidImage)),
            Ok(())
        );
        assert_eq!(
            tracker.finish(false),
            InstanceEvent::Failed(InstanceFailure::InvalidImage)
        );
    }
}

#[cfg(test)]
mod observation_tests {
    use super::{InstanceStatus, Observation, ObservationRequest};

    #[test]
    fn observation_roundtrips_and_rejects_bad_envelopes() {
        let request = ObservationRequest(u64::MAX);
        assert_eq!(ObservationRequest::decode(&request.encode()), Some(request));
        let value = Observation {
            request,
            vcpus: 4,
            capacity_bytes: 1 << 28,
            resident_bytes: Some(4096),
        };
        let encoded = value.encode();
        assert_eq!(Observation::decode(&encoded), Some(value));
        assert_eq!(InstanceStatus::decode(&encoded), None);
        assert_eq!(Observation::decode(&encoded[..39]), None);
        let mut bad = encoded;
        bad[20] = 1;
        assert_eq!(Observation::decode(&bad), None);
        // Explicit shared pools can add backing beyond the boot RAM capacity.
        let with_pool = Observation {
            resident_bytes: Some((1 << 28) + 4096),
            ..value
        };
        assert_eq!(Observation::decode(&with_pool.encode()), Some(with_pool));
        let unavailable = Observation {
            resident_bytes: None,
            ..value
        };
        assert_eq!(
            Observation::decode(&unavailable.encode()),
            Some(unavailable)
        );
    }
}

#[cfg(test)]
mod vcpu_control_tests {
    use super::*;
    #[test]
    fn affinity_and_inspection_wire_formats_are_distinct() {
        for affinity in [
            None,
            Some([0; VCPU_AFFINITY_WORDS]),
            Some([u64::MAX; VCPU_AFFINITY_WORDS]),
        ] {
            let request = VcpuControlRequest {
                sequence: u64::MAX,
                vcpu: u32::MAX,
                affinity,
            };
            let bytes = request.encode();
            assert_eq!(VcpuControlRequest::decode(&bytes), Some(request));
            assert_eq!(
                VcpuControlRequest::decode(&bytes[..CONTROL_BYTES - 1]),
                None
            );
        }
        let mut reserved = VcpuControlRequest {
            sequence: 1,
            vcpu: 0,
            affinity: None,
        }
        .encode();
        reserved[24] = 1;
        assert_eq!(VcpuControlRequest::decode(&reserved), None);
        let reply = VcpuControlReply {
            sequence: 1,
            vcpu: 0,
            host_cpu: Some(0),
            pending_host_cpu: Some(2),
            status: hyper_os::Status::OK,
        };
        let mut bytes = reply.encode();
        assert_eq!(VcpuControlReply::decode(&bytes), Some(reply));
        bytes[28] = 1;
        assert_eq!(VcpuControlReply::decode(&bytes), None);
    }
}

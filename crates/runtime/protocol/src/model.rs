use glyphshift_adapter_registry::AdapterRequirement;
use glyphshift_capture::CaptureObservationBatch;
use glyphshift_domain::{AdapterId, Feature, TargetFacts};
use glyphshift_extension::{ExtensionId, ProtocolVersion};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ControllerNonce([u8; 32]);

impl ControllerNonce {
    #[must_use]
    pub const fn new(value: [u8; 32]) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControllerHello {
    extension_id: ExtensionId,
    version: ProtocolVersion,
    nonce: ControllerNonce,
}

impl ControllerHello {
    #[must_use]
    pub const fn new(
        extension_id: ExtensionId,
        version: ProtocolVersion,
        nonce: ControllerNonce,
    ) -> Self {
        Self {
            extension_id,
            version,
            nonce,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControllerRejection {
    TargetProcessUnavailable,
    RemoteMemoryUnavailable,
    RuntimeModuleUnavailable,
    RuntimeExportUnavailable,
    RemoteThreadUnavailable,
    RemoteThreadTimeout,
    TargetRuntimeRejected(u32),
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransportFailure {
    Timeout,
    Crashed,
    MalformedMessage,
    Cancelled,
    Rejected(ControllerRejection),
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ControllerInstallationToken(Box<str>);

impl ControllerInstallationToken {
    #[must_use]
    pub fn new(value: impl Into<Box<str>>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ControllerTargetToken(Box<str>);

impl ControllerTargetToken {
    #[must_use]
    pub fn new(value: impl Into<Box<str>>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Ephemeral platform-private authority for a verified isolated worker.
///
/// The Desktop shell never receives this value. It is minted from an already authorized
/// Controller target and remains inside the runtime composition root.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControllerWorkerTargetGrant {
    platform: Box<str>,
    payload: Box<str>,
}

impl ControllerWorkerTargetGrant {
    #[must_use]
    pub fn new(platform: impl Into<Box<str>>, payload: impl Into<Box<str>>) -> Self {
        Self {
            platform: platform.into(),
            payload: payload.into(),
        }
    }

    #[must_use]
    pub fn platform(&self) -> &str {
        &self.platform
    }

    #[must_use]
    pub fn payload(&self) -> &str {
        &self.payload
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControllerInstallation {
    token: ControllerInstallationToken,
    display_name: Box<str>,
}

impl ControllerInstallation {
    #[must_use]
    pub fn new(token: ControllerInstallationToken, display_name: impl Into<Box<str>>) -> Self {
        Self {
            token,
            display_name: display_name.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControllerTarget {
    token: ControllerTargetToken,
    display_name: Box<str>,
    facts: TargetFacts,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControllerRuntimeDeployment {
    runtime_library: Box<str>,
    runtime_library_sha256: [u8; 32],
    deployment_json: Box<str>,
    generation: u64,
}

impl ControllerRuntimeDeployment {
    #[must_use]
    pub fn new(
        runtime_library: impl Into<Box<str>>,
        runtime_library_sha256: [u8; 32],
        deployment_json: impl Into<Box<str>>,
        generation: u64,
    ) -> Self {
        Self {
            runtime_library: runtime_library.into(),
            runtime_library_sha256,
            deployment_json: deployment_json.into(),
            generation,
        }
    }

    #[must_use]
    pub fn runtime_library(&self) -> &str {
        &self.runtime_library
    }

    #[must_use]
    pub const fn runtime_library_sha256(&self) -> [u8; 32] {
        self.runtime_library_sha256
    }

    #[must_use]
    pub fn deployment_json(&self) -> &str {
        &self.deployment_json
    }

    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControllerRuntimeAck {
    generation: u64,
    publication_identity: [u8; 32],
    active_adapter_ids: Option<BTreeSet<AdapterId>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControllerRuntimeTraceStatus {
    NoMatch,
    Matched,
    ContextRecorded,
    InvalidObservation,
    InvalidRouteProgram,
    ExecutionLimitExceeded,
    StateLimitExceeded,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControllerRuntimeTextOutcome {
    Unmatched,
    Replaced,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControllerRuntimeFontOutcome {
    Unmatched,
    Protected,
    Substituted,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControllerRuntimeTraceRecord {
    adapter_id: Box<str>,
    source_text: Box<str>,
    status: ControllerRuntimeTraceStatus,
    text: ControllerRuntimeTextOutcome,
    font: ControllerRuntimeFontOutcome,
    generation: u64,
    publication_identity: [u8; 32],
    translation_digest: [u8; 32],
    font_policy_digest: [u8; 32],
}

impl ControllerRuntimeTraceRecord {
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        adapter_id: impl Into<Box<str>>,
        source_text: impl Into<Box<str>>,
        status: ControllerRuntimeTraceStatus,
        text: ControllerRuntimeTextOutcome,
        font: ControllerRuntimeFontOutcome,
        generation: u64,
        publication_identity: [u8; 32],
        translation_digest: [u8; 32],
        font_policy_digest: [u8; 32],
    ) -> Self {
        Self {
            adapter_id: adapter_id.into(),
            source_text: source_text.into(),
            status,
            text,
            font,
            generation,
            publication_identity,
            translation_digest,
            font_policy_digest,
        }
    }

    #[must_use]
    pub fn adapter_id(&self) -> &str {
        &self.adapter_id
    }

    #[must_use]
    pub fn source_text(&self) -> &str {
        &self.source_text
    }

    #[must_use]
    pub const fn status(&self) -> ControllerRuntimeTraceStatus {
        self.status
    }

    #[must_use]
    pub const fn text(&self) -> ControllerRuntimeTextOutcome {
        self.text
    }

    #[must_use]
    pub const fn font(&self) -> ControllerRuntimeFontOutcome {
        self.font
    }

    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    #[must_use]
    pub const fn publication_identity(&self) -> [u8; 32] {
        self.publication_identity
    }

    #[must_use]
    pub const fn translation_digest(&self) -> [u8; 32] {
        self.translation_digest
    }

    #[must_use]
    pub const fn font_policy_digest(&self) -> [u8; 32] {
        self.font_policy_digest
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ControllerRuntimeTraceBatch {
    records: Vec<ControllerRuntimeTraceRecord>,
    dropped: u64,
}

impl ControllerRuntimeTraceBatch {
    #[must_use]
    pub fn new(
        records: impl IntoIterator<Item = ControllerRuntimeTraceRecord>,
        dropped: u64,
    ) -> Self {
        Self {
            records: records.into_iter().collect(),
            dropped,
        }
    }

    #[must_use]
    pub fn records(&self) -> &[ControllerRuntimeTraceRecord] {
        &self.records
    }

    #[must_use]
    pub const fn dropped(&self) -> u64 {
        self.dropped
    }
}

impl ControllerRuntimeAck {
    #[must_use]
    pub const fn new(generation: u64, publication_identity: [u8; 32]) -> Self {
        Self {
            generation,
            publication_identity,
            active_adapter_ids: None,
        }
    }

    #[must_use]
    pub fn reported(
        generation: u64,
        publication_identity: [u8; 32],
        active_adapter_ids: impl IntoIterator<Item = AdapterId>,
    ) -> Self {
        Self {
            generation,
            publication_identity,
            active_adapter_ids: Some(active_adapter_ids.into_iter().collect()),
        }
    }

    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    #[must_use]
    pub const fn publication_identity(&self) -> [u8; 32] {
        self.publication_identity
    }

    #[must_use]
    pub const fn active_adapter_ids(&self) -> Option<&BTreeSet<AdapterId>> {
        self.active_adapter_ids.as_ref()
    }
}

impl ControllerTarget {
    #[must_use]
    pub fn new(
        token: ControllerTargetToken,
        display_name: impl Into<Box<str>>,
        facts: TargetFacts,
    ) -> Self {
        Self {
            token,
            display_name: display_name.into(),
            facts,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ControllerInventory {
    installations: Vec<ControllerInstallation>,
    targets: Vec<ControllerTarget>,
}

impl ControllerInventory {
    #[must_use]
    pub fn new(
        installations: impl IntoIterator<Item = ControllerInstallation>,
        targets: impl IntoIterator<Item = ControllerTarget>,
    ) -> Self {
        Self {
            installations: installations.into_iter().collect(),
            targets: targets.into_iter().collect(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InstallationId(u64);

impl InstallationId {
    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OpaqueTargetId(u64);

impl OpaqueTargetId {
    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscoveredInstallation {
    id: InstallationId,
    display_name: Box<str>,
}

impl DiscoveredInstallation {
    #[must_use]
    pub const fn id(&self) -> InstallationId {
        self.id
    }

    #[must_use]
    pub fn display_name(&self) -> &str {
        &self.display_name
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscoveredTarget {
    id: OpaqueTargetId,
    display_name: Box<str>,
    facts: TargetFacts,
}

impl DiscoveredTarget {
    #[must_use]
    pub const fn id(&self) -> OpaqueTargetId {
        self.id
    }

    #[must_use]
    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    #[must_use]
    pub const fn facts(&self) -> &TargetFacts {
        &self.facts
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InventoryView {
    installations: Vec<DiscoveredInstallation>,
    targets: Vec<DiscoveredTarget>,
}

impl InventoryView {
    #[must_use]
    pub fn installations(&self) -> &[DiscoveredInstallation] {
        &self.installations
    }

    #[must_use]
    pub fn targets(&self) -> &[DiscoveredTarget] {
        &self.targets
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ControllerLaunchAck;

impl ControllerLaunchAck {
    #[must_use]
    pub const fn accepted() -> Self {
        Self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LaunchState {
    Requested,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LaunchReceipt {
    installation_id: InstallationId,
    state: LaunchState,
}

impl LaunchReceipt {
    #[must_use]
    pub const fn installation_id(self) -> InstallationId {
        self.installation_id
    }

    #[must_use]
    pub const fn state(self) -> LaunchState {
        self.state
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecipeControllerLossPolicy {
    Continue,
    Degrade,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RecipeDirective {
    Adapter(AdapterRequirement),
    FunctionAddress(u64),
    HookCode,
    Script,
}

impl RecipeDirective {
    #[must_use]
    pub const fn adapter(requirement: AdapterRequirement) -> Self {
        Self::Adapter(requirement)
    }

    #[must_use]
    pub const fn function_address(address: u64) -> Self {
        Self::FunctionAddress(address)
    }

    #[must_use]
    pub const fn hook_code() -> Self {
        Self::HookCode
    }

    #[must_use]
    pub const fn script() -> Self {
        Self::Script
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControllerRecipe {
    directives: Vec<RecipeDirective>,
    controller_loss_policy: RecipeControllerLossPolicy,
}

impl ControllerRecipe {
    #[must_use]
    pub fn new(
        directives: impl IntoIterator<Item = RecipeDirective>,
        controller_loss_policy: RecipeControllerLossPolicy,
    ) -> Self {
        Self {
            directives: directives.into_iter().collect(),
            controller_loss_policy,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreparedRecipe {
    requirements: Vec<AdapterRequirement>,
    controller_loss_policy: RecipeControllerLossPolicy,
}

impl PreparedRecipe {
    #[must_use]
    pub fn requirements(&self) -> &[AdapterRequirement] {
        &self.requirements
    }

    #[must_use]
    pub const fn controller_loss_policy(&self) -> RecipeControllerLossPolicy {
        self.controller_loss_policy
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ControllerOperation {
    Inventory,
    Prepare(ControllerTargetToken),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RecipeViolation {
    FunctionAddress,
    HookCode,
    Script,
    UnknownAdapter(AdapterId),
    UnauthorizedFeature {
        adapter_id: AdapterId,
        feature: Feature,
    },
}

mod architecture;
mod connection;
pub use architecture::ArchitectureControllerTransport;

pub use connection::*;

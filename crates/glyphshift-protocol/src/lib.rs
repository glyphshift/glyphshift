//! Controller Plugin protocol state and validation.

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

pub trait ControllerTransport {
    fn handshake(
        &mut self,
        expected_extension: &ExtensionId,
        version: ProtocolVersion,
        nonce: ControllerNonce,
    ) -> Result<ControllerHello, TransportFailure>;

    fn inventory(&mut self) -> Result<ControllerInventory, TransportFailure> {
        Err(TransportFailure::MalformedMessage)
    }

    fn launch(
        &mut self,
        _installation: &ControllerInstallationToken,
    ) -> Result<ControllerLaunchAck, TransportFailure> {
        Err(TransportFailure::MalformedMessage)
    }

    fn prepare(
        &mut self,
        _target: &ControllerTargetToken,
        _requested_features: &BTreeSet<Feature>,
    ) -> Result<ControllerRecipe, TransportFailure> {
        Err(TransportFailure::MalformedMessage)
    }

    fn authorize_worker_target(
        &mut self,
        _target: &ControllerTargetToken,
    ) -> Result<ControllerWorkerTargetGrant, TransportFailure> {
        Err(TransportFailure::MalformedMessage)
    }

    fn activate_runtime(
        &mut self,
        _target: &ControllerTargetToken,
        _deployment: &ControllerRuntimeDeployment,
    ) -> Result<ControllerRuntimeAck, TransportFailure> {
        Err(TransportFailure::MalformedMessage)
    }

    fn update_runtime(
        &mut self,
        _target: &ControllerTargetToken,
        _publication_json: &str,
        _generation: u64,
    ) -> Result<ControllerRuntimeAck, TransportFailure> {
        Err(TransportFailure::MalformedMessage)
    }

    fn control_capture(
        &mut self,
        _target: &ControllerTargetToken,
        _paused: bool,
    ) -> Result<(), TransportFailure> {
        Err(TransportFailure::MalformedMessage)
    }

    fn control_runtime_diagnostics(
        &mut self,
        _target: &ControllerTargetToken,
        _enabled: bool,
    ) -> Result<(), TransportFailure> {
        Err(TransportFailure::MalformedMessage)
    }

    fn query_runtime_diagnostics(
        &mut self,
        _target: &ControllerTargetToken,
    ) -> Result<ControllerRuntimeTraceBatch, TransportFailure> {
        Err(TransportFailure::MalformedMessage)
    }

    fn query_observations(
        &mut self,
        _target: &ControllerTargetToken,
    ) -> Result<CaptureObservationBatch, TransportFailure> {
        Err(TransportFailure::MalformedMessage)
    }

    fn deactivate_runtime(
        &mut self,
        _target: &ControllerTargetToken,
    ) -> Result<(), TransportFailure> {
        Err(TransportFailure::MalformedMessage)
    }

    fn cancel(&mut self, _operation: &ControllerOperation) -> Result<(), TransportFailure> {
        Err(TransportFailure::MalformedMessage)
    }

    fn terminate(&mut self);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControllerHealth {
    Available,
    Degraded,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ControllerProtocolError {
    NonceReplay(ControllerNonce),
    VersionMismatch,
    NonceMismatch,
    ExtensionMismatch,
    UnknownInstallation(InstallationId),
    UnknownTarget(OpaqueTargetId),
    InvalidRecipe(RecipeViolation),
    Transport(TransportFailure),
}

#[derive(Debug, Default)]
pub struct NonceLedger {
    accepted: BTreeSet<ControllerNonce>,
}

impl NonceLedger {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            accepted: BTreeSet::new(),
        }
    }
}

#[derive(Debug)]
pub struct ControllerConnection<T> {
    transport: T,
    extension_id: ExtensionId,
    health: ControllerHealth,
    installation_tokens: BTreeMap<InstallationId, ControllerInstallationToken>,
    target_tokens: BTreeMap<OpaqueTargetId, ControllerTargetToken>,
    authorized_requirements: Vec<AdapterRequirement>,
    cancelled_inventory: bool,
    cancelled_targets: BTreeSet<OpaqueTargetId>,
}

impl<T: ControllerTransport> ControllerConnection<T> {
    pub fn connect(
        mut transport: T,
        extension_id: ExtensionId,
        version: ProtocolVersion,
        nonce: ControllerNonce,
        ledger: &mut NonceLedger,
    ) -> Result<Self, ControllerProtocolError> {
        if ledger.accepted.contains(&nonce) {
            transport.terminate();
            return Err(ControllerProtocolError::NonceReplay(nonce));
        }
        let hello = transport
            .handshake(&extension_id, version, nonce)
            .map_err(ControllerProtocolError::Transport)?;
        let rejection = if hello.version != version {
            Some(ControllerProtocolError::VersionMismatch)
        } else if hello.nonce != nonce {
            Some(ControllerProtocolError::NonceMismatch)
        } else if hello.extension_id != extension_id {
            Some(ControllerProtocolError::ExtensionMismatch)
        } else {
            None
        };
        if let Some(rejection) = rejection {
            transport.terminate();
            return Err(rejection);
        }
        ledger.accepted.insert(nonce);
        Ok(Self {
            transport,
            extension_id,
            health: ControllerHealth::Available,
            installation_tokens: BTreeMap::new(),
            target_tokens: BTreeMap::new(),
            authorized_requirements: Vec::new(),
            cancelled_inventory: false,
            cancelled_targets: BTreeSet::new(),
        })
    }

    #[must_use]
    pub const fn extension_id(&self) -> &ExtensionId {
        &self.extension_id
    }

    #[must_use]
    pub const fn health(&self) -> ControllerHealth {
        self.health
    }

    pub fn inventory(&mut self) -> Result<InventoryView, ControllerProtocolError> {
        if std::mem::take(&mut self.cancelled_inventory) {
            return Err(ControllerProtocolError::Transport(
                TransportFailure::Cancelled,
            ));
        }
        let inventory = match self.transport.inventory() {
            Ok(inventory) => inventory,
            Err(failure) => return Err(self.handle_transport_failure(failure)),
        };
        self.installation_tokens.clear();
        self.target_tokens.clear();
        let installations = inventory
            .installations
            .into_iter()
            .enumerate()
            .map(|(index, installation)| {
                let id = InstallationId(index as u64 + 1);
                self.installation_tokens
                    .insert(id, installation.token.clone());
                DiscoveredInstallation {
                    id,
                    display_name: installation.display_name,
                }
            })
            .collect();
        let targets = inventory
            .targets
            .into_iter()
            .enumerate()
            .map(|(index, target)| {
                let id = OpaqueTargetId(index as u64 + 1);
                self.target_tokens.insert(id, target.token.clone());
                DiscoveredTarget {
                    id,
                    display_name: target.display_name,
                    facts: target.facts,
                }
            })
            .collect();
        Ok(InventoryView {
            installations,
            targets,
        })
    }

    pub fn launch(
        &mut self,
        installation_id: InstallationId,
    ) -> Result<LaunchReceipt, ControllerProtocolError> {
        let token = self
            .installation_tokens
            .get(&installation_id)
            .cloned()
            .ok_or(ControllerProtocolError::UnknownInstallation(
                installation_id,
            ))?;
        if let Err(failure) = self.transport.launch(&token) {
            return Err(self.handle_transport_failure(failure));
        }
        Ok(LaunchReceipt {
            installation_id,
            state: LaunchState::Requested,
        })
    }

    pub fn authorize_capabilities(
        &mut self,
        requirements: impl IntoIterator<Item = AdapterRequirement>,
    ) {
        self.authorized_requirements = requirements.into_iter().collect();
    }

    pub fn prepare(
        &mut self,
        target_id: OpaqueTargetId,
        requested_features: impl IntoIterator<Item = Feature>,
    ) -> Result<PreparedRecipe, ControllerProtocolError> {
        if self.cancelled_targets.remove(&target_id) {
            return Err(ControllerProtocolError::Transport(
                TransportFailure::Cancelled,
            ));
        }
        let target = self
            .target_tokens
            .get(&target_id)
            .cloned()
            .ok_or(ControllerProtocolError::UnknownTarget(target_id))?;
        let requested_features: BTreeSet<_> = requested_features.into_iter().collect();
        let recipe = match self.transport.prepare(&target, &requested_features) {
            Ok(recipe) => recipe,
            Err(failure) => return Err(self.handle_transport_failure(failure)),
        };
        let mut requirements = Vec::new();
        for directive in recipe.directives {
            let requirement = match directive {
                RecipeDirective::Adapter(requirement) => requirement,
                RecipeDirective::FunctionAddress(_) => {
                    return Err(ControllerProtocolError::InvalidRecipe(
                        RecipeViolation::FunctionAddress,
                    ));
                }
                RecipeDirective::HookCode => {
                    return Err(ControllerProtocolError::InvalidRecipe(
                        RecipeViolation::HookCode,
                    ));
                }
                RecipeDirective::Script => {
                    return Err(ControllerProtocolError::InvalidRecipe(
                        RecipeViolation::Script,
                    ));
                }
            };
            let authorized = self.authorized_requirements.iter().find(|authorized| {
                authorized.adapter_id() == requirement.adapter_id()
                    && authorized.version_requirement() == requirement.version_requirement()
            });
            let Some(authorized) = authorized else {
                return Err(ControllerProtocolError::InvalidRecipe(
                    RecipeViolation::UnknownAdapter(requirement.adapter_id().clone()),
                ));
            };
            for feature in requirement.features() {
                if !authorized
                    .features()
                    .any(|authorized_feature| authorized_feature == feature)
                {
                    return Err(ControllerProtocolError::InvalidRecipe(
                        RecipeViolation::UnauthorizedFeature {
                            adapter_id: requirement.adapter_id().clone(),
                            feature,
                        },
                    ));
                }
            }
            requirements.push(requirement);
        }
        Ok(PreparedRecipe {
            requirements,
            controller_loss_policy: recipe.controller_loss_policy,
        })
    }

    pub fn activate_runtime(
        &mut self,
        target_id: OpaqueTargetId,
        deployment: &ControllerRuntimeDeployment,
    ) -> Result<ControllerRuntimeAck, ControllerProtocolError> {
        let target = self
            .target_tokens
            .get(&target_id)
            .cloned()
            .ok_or(ControllerProtocolError::UnknownTarget(target_id))?;
        self.transport
            .activate_runtime(&target, deployment)
            .map_err(|failure| self.handle_transport_failure(failure))
    }

    pub fn authorize_worker_target(
        &mut self,
        target_id: OpaqueTargetId,
    ) -> Result<ControllerWorkerTargetGrant, ControllerProtocolError> {
        let target = self
            .target_tokens
            .get(&target_id)
            .cloned()
            .ok_or(ControllerProtocolError::UnknownTarget(target_id))?;
        self.transport
            .authorize_worker_target(&target)
            .map_err(|failure| self.handle_transport_failure(failure))
    }

    pub fn update_runtime(
        &mut self,
        target_id: OpaqueTargetId,
        publication_json: &str,
        generation: u64,
    ) -> Result<ControllerRuntimeAck, ControllerProtocolError> {
        let target = self
            .target_tokens
            .get(&target_id)
            .cloned()
            .ok_or(ControllerProtocolError::UnknownTarget(target_id))?;
        self.transport
            .update_runtime(&target, publication_json, generation)
            .map_err(|failure| self.handle_transport_failure(failure))
    }

    pub fn control_capture(
        &mut self,
        target_id: OpaqueTargetId,
        paused: bool,
    ) -> Result<(), ControllerProtocolError> {
        let target = self
            .target_tokens
            .get(&target_id)
            .cloned()
            .ok_or(ControllerProtocolError::UnknownTarget(target_id))?;
        self.transport
            .control_capture(&target, paused)
            .map_err(|failure| self.handle_transport_failure(failure))
    }

    pub fn control_runtime_diagnostics(
        &mut self,
        target_id: OpaqueTargetId,
        enabled: bool,
    ) -> Result<(), ControllerProtocolError> {
        let target = self
            .target_tokens
            .get(&target_id)
            .cloned()
            .ok_or(ControllerProtocolError::UnknownTarget(target_id))?;
        self.transport
            .control_runtime_diagnostics(&target, enabled)
            .map_err(|failure| self.handle_transport_failure(failure))
    }

    pub fn query_runtime_diagnostics(
        &mut self,
        target_id: OpaqueTargetId,
    ) -> Result<ControllerRuntimeTraceBatch, ControllerProtocolError> {
        let target = self
            .target_tokens
            .get(&target_id)
            .cloned()
            .ok_or(ControllerProtocolError::UnknownTarget(target_id))?;
        self.transport
            .query_runtime_diagnostics(&target)
            .map_err(|failure| self.handle_transport_failure(failure))
    }

    pub fn query_observations(
        &mut self,
        target_id: OpaqueTargetId,
    ) -> Result<CaptureObservationBatch, ControllerProtocolError> {
        let target = self
            .target_tokens
            .get(&target_id)
            .cloned()
            .ok_or(ControllerProtocolError::UnknownTarget(target_id))?;
        self.transport
            .query_observations(&target)
            .map_err(|failure| self.handle_transport_failure(failure))
    }

    pub fn deactivate_runtime(
        &mut self,
        target_id: OpaqueTargetId,
    ) -> Result<(), ControllerProtocolError> {
        let target = self
            .target_tokens
            .get(&target_id)
            .cloned()
            .ok_or(ControllerProtocolError::UnknownTarget(target_id))?;
        self.transport
            .deactivate_runtime(&target)
            .map_err(|failure| self.handle_transport_failure(failure))
    }

    pub fn cancel_inventory(&mut self) -> Result<(), ControllerProtocolError> {
        if let Err(failure) = self.transport.cancel(&ControllerOperation::Inventory) {
            return Err(self.handle_transport_failure(failure));
        }
        self.cancelled_inventory = true;
        Ok(())
    }

    pub fn cancel_prepare(
        &mut self,
        target_id: OpaqueTargetId,
    ) -> Result<(), ControllerProtocolError> {
        let target = self
            .target_tokens
            .get(&target_id)
            .cloned()
            .ok_or(ControllerProtocolError::UnknownTarget(target_id))?;
        if let Err(failure) = self.transport.cancel(&ControllerOperation::Prepare(target)) {
            return Err(self.handle_transport_failure(failure));
        }
        self.cancelled_targets.insert(target_id);
        Ok(())
    }

    fn handle_transport_failure(&mut self, failure: TransportFailure) -> ControllerProtocolError {
        if !matches!(
            failure,
            TransportFailure::Cancelled | TransportFailure::Rejected(_)
        ) {
            self.health = ControllerHealth::Degraded;
            self.transport.terminate();
        }
        ControllerProtocolError::Transport(failure)
    }

    #[must_use]
    pub fn into_transport(self) -> T {
        self.transport
    }
}

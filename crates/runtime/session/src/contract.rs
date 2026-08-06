use glyphshift_adapter_registry::{
    AdapterBinding, AdapterRequirement, AdapterVersion, RegistryError,
};
use glyphshift_domain::{AdapterId, Feature, Generation, TargetFacts};
use glyphshift_runtime_contract::RuntimePublication;
use std::collections::{BTreeMap, BTreeSet};

const SESSION_DIAGNOSTIC_LIMIT: usize = 64;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TargetInstanceId(Box<str>);

impl TargetInstanceId {
    #[must_use]
    pub fn new(value: impl Into<Box<str>>) -> Self {
        Self(value.into())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TargetInstance {
    id: TargetInstanceId,
    facts: TargetFacts,
}

impl TargetInstance {
    #[must_use]
    pub const fn new(id: TargetInstanceId, facts: TargetFacts) -> Self {
        Self { id, facts }
    }

    #[must_use]
    pub const fn id(&self) -> &TargetInstanceId {
        &self.id
    }

    #[must_use]
    pub const fn facts(&self) -> &TargetFacts {
        &self.facts
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SessionId(u64);

impl SessionId {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionRecipe {
    pub(super) requirements: Vec<AdapterRequirement>,
    pub(super) controller_loss_policy: ControllerLossPolicy,
}

impl SessionRecipe {
    #[must_use]
    pub fn new(
        requirements: impl IntoIterator<Item = AdapterRequirement>,
        controller_loss_policy: ControllerLossPolicy,
    ) -> Self {
        Self {
            requirements: requirements.into_iter().collect(),
            controller_loss_policy,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControllerFailure {
    Unavailable,
    InvalidRecipe,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControllerHealth {
    Available,
    Lost,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControllerLossPolicy {
    Continue,
    Degrade,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetHealth {
    Running,
    Exited,
}

pub trait ControllerRecipePort: Send {
    fn prepare(
        &mut self,
        target: &TargetInstance,
        requested_features: &BTreeSet<Feature>,
    ) -> Result<SessionRecipe, ControllerFailure>;

    fn health(&mut self, _target: &TargetInstance) -> ControllerHealth {
        ControllerHealth::Available
    }
}

pub trait TargetLifecyclePort: Send {
    fn health(&mut self, target: &TargetInstance) -> TargetHealth;
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BoundAdapter {
    adapter_id: AdapterId,
    version: AdapterVersion,
}

impl BoundAdapter {
    #[must_use]
    pub const fn new(adapter_id: AdapterId, version: AdapterVersion) -> Self {
        Self {
            adapter_id,
            version,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeactivationMode {
    PassThrough,
    RestoreOriginal,
    StopWriteback,
    StopObserving,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdapterDeactivation {
    adapter: BoundAdapter,
    mode: DeactivationMode,
}

impl AdapterDeactivation {
    #[must_use]
    pub const fn new(adapter: BoundAdapter, mode: DeactivationMode) -> Self {
        Self { adapter, mode }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BoundFeature {
    pub(super) adapter_id: AdapterId,
    pub(super) version: AdapterVersion,
    feature: Feature,
}

impl BoundFeature {
    #[must_use]
    pub const fn new(adapter_id: AdapterId, version: AdapterVersion, feature: Feature) -> Self {
        Self {
            adapter_id,
            version,
            feature,
        }
    }

    #[must_use]
    pub const fn adapter_id(&self) -> &AdapterId {
        &self.adapter_id
    }

    pub(super) fn from_binding(binding: &AdapterBinding, feature: Feature) -> Self {
        Self::new(binding.adapter_id.clone(), binding.version, feature)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostActivation {
    pub(super) acknowledged_features: BTreeSet<BoundFeature>,
    pub(super) failed_features: BTreeSet<BoundFeature>,
}

impl HostActivation {
    #[must_use]
    pub fn connected(acknowledged_features: impl IntoIterator<Item = BoundFeature>) -> Self {
        Self {
            acknowledged_features: acknowledged_features.into_iter().collect(),
            failed_features: BTreeSet::new(),
        }
    }

    #[must_use]
    pub fn reported(
        acknowledged_features: impl IntoIterator<Item = BoundFeature>,
        failed_features: impl IntoIterator<Item = BoundFeature>,
    ) -> Self {
        Self {
            acknowledged_features: acknowledged_features.into_iter().collect(),
            failed_features: failed_features.into_iter().collect(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostOperationFailure {
    TargetProcessUnavailable,
    RemoteMemoryUnavailable,
    RuntimeModuleUnavailable,
    RuntimeExportUnavailable,
    RemoteThreadUnavailable,
    RemoteThreadTimeout,
    IsolatedWorkerPermissionDenied,
    IsolatedWorkerTimeout,
    TargetRuntimeRejected(u32),
    ControllerRejected,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostFailure {
    Unavailable,
    HandshakeRejected,
    OperationRejected(HostOperationFailure),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeTraceStatus {
    NoMatch,
    Matched,
    ContextRecorded,
    InvalidObservation,
    InvalidRouteProgram,
    ExecutionLimitExceeded,
    StateLimitExceeded,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeTextOutcome {
    Unmatched,
    Replaced,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeFontOutcome {
    Unmatched,
    Protected,
    Substituted,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeTraceRecord {
    adapter_id: Box<str>,
    source_text: Box<str>,
    status: RuntimeTraceStatus,
    text: RuntimeTextOutcome,
    font: RuntimeFontOutcome,
    generation: u64,
    publication_identity: [u8; 32],
    translation_digest: [u8; 32],
    font_policy_digest: [u8; 32],
}

impl RuntimeTraceRecord {
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        adapter_id: impl Into<Box<str>>,
        source_text: impl Into<Box<str>>,
        status: RuntimeTraceStatus,
        text: RuntimeTextOutcome,
        font: RuntimeFontOutcome,
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
    pub const fn status(&self) -> RuntimeTraceStatus {
        self.status
    }

    #[must_use]
    pub const fn text(&self) -> RuntimeTextOutcome {
        self.text
    }

    #[must_use]
    pub const fn font(&self) -> RuntimeFontOutcome {
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
pub struct RuntimeTraceBatch {
    records: Vec<RuntimeTraceRecord>,
    dropped: u64,
}

impl RuntimeTraceBatch {
    #[must_use]
    pub fn new(records: impl IntoIterator<Item = RuntimeTraceRecord>, dropped: u64) -> Self {
        Self {
            records: records.into_iter().collect(),
            dropped,
        }
    }

    #[must_use]
    pub fn records(&self) -> &[RuntimeTraceRecord] {
        &self.records
    }

    #[must_use]
    pub const fn dropped(&self) -> u64 {
        self.dropped
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HostGenerationReport {
    pub(super) target_runtime_ack: Option<Generation>,
    pub(super) isolated_feature_acks: BTreeMap<BoundFeature, Generation>,
}

impl HostGenerationReport {
    #[must_use]
    pub const fn target_runtime(generation: Generation) -> Self {
        Self {
            target_runtime_ack: Some(generation),
            isolated_feature_acks: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn isolated(
        acknowledgements: impl IntoIterator<Item = (BoundFeature, Generation)>,
    ) -> Self {
        Self {
            target_runtime_ack: None,
            isolated_feature_acks: acknowledgements.into_iter().collect(),
        }
    }

    #[must_use]
    pub fn reported(
        target_runtime_ack: Option<Generation>,
        isolated_acknowledgements: impl IntoIterator<Item = (BoundFeature, Generation)>,
    ) -> Self {
        Self {
            target_runtime_ack,
            isolated_feature_acks: isolated_acknowledgements.into_iter().collect(),
        }
    }

    #[must_use]
    pub const fn pending() -> Self {
        Self {
            target_runtime_ack: None,
            isolated_feature_acks: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HostHealthReport {
    pub(super) failed_adapters: BTreeSet<BoundAdapter>,
    pub(super) diagnostics: Vec<SessionDiagnostic>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionDiagnostic(Box<str>);

impl SessionDiagnostic {
    #[must_use]
    pub fn new(code: impl Into<Box<str>>) -> Self {
        Self(code.into())
    }

    #[must_use]
    pub fn code(&self) -> &str {
        &self.0
    }
}

impl HostHealthReport {
    #[must_use]
    pub const fn healthy() -> Self {
        Self {
            failed_adapters: BTreeSet::new(),
            diagnostics: Vec::new(),
        }
    }

    #[must_use]
    pub fn failed_adapters(&self) -> &BTreeSet<BoundAdapter> {
        &self.failed_adapters
    }

    #[must_use]
    pub fn diagnostics(&self) -> &[SessionDiagnostic] {
        &self.diagnostics
    }

    #[must_use]
    pub fn failed(failed_adapters: impl IntoIterator<Item = BoundAdapter>) -> Self {
        Self {
            failed_adapters: failed_adapters.into_iter().collect(),
            diagnostics: Vec::new(),
        }
    }

    #[must_use]
    pub fn reported(
        failed_adapters: impl IntoIterator<Item = BoundAdapter>,
        diagnostics: impl IntoIterator<Item = SessionDiagnostic>,
    ) -> Self {
        Self {
            failed_adapters: failed_adapters.into_iter().collect(),
            diagnostics: diagnostics
                .into_iter()
                .take(SESSION_DIAGNOSTIC_LIMIT)
                .collect(),
        }
    }

    #[must_use]
    pub fn merge(mut self, other: Self) -> Self {
        self.failed_adapters.extend(other.failed_adapters);
        self.diagnostics.extend(other.diagnostics);
        self.diagnostics.truncate(SESSION_DIAGNOSTIC_LIMIT);
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HostDeactivation {
    pub(super) completed_adapters: BTreeSet<BoundAdapter>,
    pub(super) failed_adapters: BTreeSet<BoundAdapter>,
}

impl HostDeactivation {
    #[must_use]
    pub fn completed(completed_adapters: impl IntoIterator<Item = BoundAdapter>) -> Self {
        Self {
            completed_adapters: completed_adapters.into_iter().collect(),
            failed_adapters: BTreeSet::new(),
        }
    }

    #[must_use]
    pub fn reported(
        completed_adapters: impl IntoIterator<Item = BoundAdapter>,
        failed_adapters: impl IntoIterator<Item = BoundAdapter>,
    ) -> Self {
        Self {
            completed_adapters: completed_adapters.into_iter().collect(),
            failed_adapters: failed_adapters.into_iter().collect(),
        }
    }

    #[must_use]
    pub const fn pending() -> Self {
        Self {
            completed_adapters: BTreeSet::new(),
            failed_adapters: BTreeSet::new(),
        }
    }

    #[must_use]
    pub fn merge(mut self, other: Self) -> Self {
        self.completed_adapters.extend(other.completed_adapters);
        self.failed_adapters.extend(other.failed_adapters);
        self
    }
}

pub trait AdapterHostPort: Send {
    fn activate(
        &mut self,
        target: &TargetInstance,
        bindings: &[AdapterBinding],
    ) -> Result<HostActivation, HostFailure>;

    fn activate_runtime(
        &mut self,
        target: &TargetInstance,
        bindings: &[AdapterBinding],
        _publication: &RuntimePublication,
    ) -> Result<HostActivation, HostFailure> {
        self.activate(target, bindings)
    }

    fn update(
        &mut self,
        _session_id: SessionId,
        _target: &TargetInstance,
        _bindings: &[AdapterBinding],
        _generation: Generation,
    ) -> Result<HostGenerationReport, HostFailure> {
        Ok(HostGenerationReport::pending())
    }

    fn update_runtime(
        &mut self,
        session_id: SessionId,
        target: &TargetInstance,
        bindings: &[AdapterBinding],
        publication: &RuntimePublication,
    ) -> Result<HostGenerationReport, HostFailure> {
        self.update(session_id, target, bindings, publication.generation())
    }

    fn control_capture(
        &mut self,
        _session_id: SessionId,
        _target: &TargetInstance,
        _paused: bool,
    ) -> Result<(), HostFailure> {
        Err(HostFailure::Unavailable)
    }

    fn control_runtime_diagnostics(
        &mut self,
        _session_id: SessionId,
        _target: &TargetInstance,
        _enabled: bool,
    ) -> Result<(), HostFailure> {
        Err(HostFailure::Unavailable)
    }

    fn query_runtime_diagnostics(
        &mut self,
        _session_id: SessionId,
        _target: &TargetInstance,
    ) -> Result<RuntimeTraceBatch, HostFailure> {
        Err(HostFailure::Unavailable)
    }

    fn health(
        &mut self,
        _session_id: SessionId,
        _target: &TargetInstance,
        _bindings: &[AdapterBinding],
    ) -> Result<HostHealthReport, HostFailure> {
        Ok(HostHealthReport::healthy())
    }

    fn deactivate(
        &mut self,
        _session_id: SessionId,
        _target: &TargetInstance,
        _bindings: &[AdapterBinding],
        _plan: &[AdapterDeactivation],
    ) -> Result<HostDeactivation, HostFailure> {
        Ok(HostDeactivation::pending())
    }

    fn release(
        &mut self,
        _session_id: SessionId,
        _target: &TargetInstance,
        _bindings: &[AdapterBinding],
    ) -> Result<(), HostFailure> {
        Err(HostFailure::Unavailable)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FeaturePhase {
    Ready,
    Starting,
    Active,
    Degraded,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GenerationPhase {
    Unreported,
    Updating {
        desired: Generation,
        applied: Option<Generation>,
    },
    Applied(Generation),
    Mismatch {
        desired: Generation,
        acknowledged: Generation,
        applied: Option<Generation>,
    },
}

impl GenerationPhase {
    pub(super) const fn applied(self) -> Option<Generation> {
        match self {
            Self::Unreported => None,
            Self::Updating { applied, .. } | Self::Mismatch { applied, .. } => applied,
            Self::Applied(generation) => Some(generation),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionStatus {
    pub(super) session_id: SessionId,
    pub(super) target_instance_id: TargetInstanceId,
    pub(super) features: BTreeMap<BoundFeature, FeaturePhase>,
    pub(super) generations: BTreeMap<BoundFeature, GenerationPhase>,
    pub(super) diagnostics: Vec<SessionDiagnostic>,
}

impl SessionStatus {
    #[must_use]
    pub const fn session_id(&self) -> SessionId {
        self.session_id
    }

    #[must_use]
    pub const fn target_instance_id(&self) -> &TargetInstanceId {
        &self.target_instance_id
    }

    #[must_use]
    pub fn phase(&self, feature: &BoundFeature) -> Option<FeaturePhase> {
        self.features.get(feature).copied()
    }

    /// Capabilities the target host actually acknowledged for this session.
    ///
    /// Requested, starting, degraded, and failed features are intentionally excluded so callers
    /// do not mistake configuration intent for a working runtime capability.
    pub fn active_features(&self) -> impl Iterator<Item = Feature> + '_ {
        self.features.iter().filter_map(|(feature, phase)| {
            (*phase == FeaturePhase::Active).then_some(feature.feature)
        })
    }

    #[must_use]
    pub fn generation(&self, feature: &BoundFeature) -> Option<GenerationPhase> {
        self.generations.get(feature).copied()
    }

    #[must_use]
    pub fn diagnostics(&self) -> &[SessionDiagnostic] {
        &self.diagnostics
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionError {
    Controller(ControllerFailure),
    Adapter(RegistryError),
    Host(HostFailure),
    ConflictingHostReport(BoundFeature),
    ConflictingHostDeactivation(BoundAdapter),
    TargetReleaseFailed {
        session_id: SessionId,
        failure: HostFailure,
    },
    TargetExited(SessionId),
    FeaturesUnavailable(Vec<BoundFeature>),
    RequestedFeatureUnavailable(Feature),
    SessionNotFound(SessionId),
    SessionIdExhausted,
}

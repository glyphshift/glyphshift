//! Session orchestration for explicit target instances.

use glyphshift_adapter_registry::{
    AdapterBinding, AdapterHostBinding, AdapterRegistry, AdapterRequirement, AdapterVersion,
    AdapterVersionRequirement, RegistryError,
};
use glyphshift_domain::{AdapterId, ApplyModel, Feature, Generation, TargetFacts};
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
    requirements: Vec<AdapterRequirement>,
    controller_loss_policy: ControllerLossPolicy,
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
    adapter_id: AdapterId,
    version: AdapterVersion,
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

    fn from_binding(binding: &AdapterBinding, feature: Feature) -> Self {
        Self::new(binding.adapter_id.clone(), binding.version, feature)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostActivation {
    acknowledged_features: BTreeSet<BoundFeature>,
    failed_features: BTreeSet<BoundFeature>,
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
    target_runtime_ack: Option<Generation>,
    isolated_feature_acks: BTreeMap<BoundFeature, Generation>,
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
    pub const fn pending() -> Self {
        Self {
            target_runtime_ack: None,
            isolated_feature_acks: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HostHealthReport {
    failed_adapters: BTreeSet<BoundAdapter>,
    diagnostics: Vec<SessionDiagnostic>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionDiagnostic(Box<str>);

impl SessionDiagnostic {
    #[must_use]
    pub fn new(code: impl Into<Box<str>>) -> Self {
        Self(code.into())
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
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HostDeactivation {
    completed_adapters: BTreeSet<BoundAdapter>,
    failed_adapters: BTreeSet<BoundAdapter>,
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
    const fn applied(self) -> Option<Generation> {
        match self {
            Self::Unreported => None,
            Self::Updating { applied, .. } | Self::Mismatch { applied, .. } => applied,
            Self::Applied(generation) => Some(generation),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionStatus {
    session_id: SessionId,
    target_instance_id: TargetInstanceId,
    features: BTreeMap<BoundFeature, FeaturePhase>,
    generations: BTreeMap<BoundFeature, GenerationPhase>,
    diagnostics: Vec<SessionDiagnostic>,
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

struct SessionRecord {
    target: TargetInstance,
    bindings: Vec<AdapterBinding>,
    controller_loss_policy: ControllerLossPolicy,
    status: SessionStatus,
}

pub struct SessionManager {
    registry: AdapterRegistry,
    controller: Box<dyn ControllerRecipePort>,
    host: Box<dyn AdapterHostPort>,
    target_lifecycle: Box<dyn TargetLifecyclePort>,
    last_session_id: u64,
    sessions: BTreeMap<SessionId, SessionRecord>,
}

impl SessionManager {
    #[must_use]
    pub fn new(
        registry: AdapterRegistry,
        controller: impl ControllerRecipePort + 'static,
        host: impl AdapterHostPort + 'static,
        target_lifecycle: impl TargetLifecyclePort + 'static,
    ) -> Self {
        Self {
            registry,
            controller: Box::new(controller),
            host: Box::new(host),
            target_lifecycle: Box::new(target_lifecycle),
            last_session_id: 0,
            sessions: BTreeMap::new(),
        }
    }

    pub fn start(
        &mut self,
        target: TargetInstance,
        requested_features: impl IntoIterator<Item = Feature>,
    ) -> Result<SessionStatus, SessionError> {
        self.start_inner(target, requested_features, None)
    }

    pub fn start_with_runtime(
        &mut self,
        target: TargetInstance,
        requested_features: impl IntoIterator<Item = Feature>,
        publication: &RuntimePublication,
    ) -> Result<SessionStatus, SessionError> {
        self.start_inner(target, requested_features, Some(publication))
    }

    fn start_inner(
        &mut self,
        target: TargetInstance,
        requested_features: impl IntoIterator<Item = Feature>,
        publication: Option<&RuntimePublication>,
    ) -> Result<SessionStatus, SessionError> {
        let requested_features: BTreeSet<_> = requested_features.into_iter().collect();
        let recipe = self
            .controller
            .prepare(&target, &requested_features)
            .map_err(SessionError::Controller)?;

        let bindings: Vec<_> = recipe
            .requirements
            .iter()
            .map(|requirement| {
                self.registry
                    .resolve(requirement, target.facts())
                    .map_err(|error| {
                        if matches!(
                            error,
                            RegistryError::AdapterNotFound(_)
                                | RegistryError::AdapterVersionNotFound { .. }
                        ) {
                            let AdapterVersionRequirement::Exact(version) =
                                requirement.version_requirement();
                            SessionError::FeaturesUnavailable(
                                requirement
                                    .features()
                                    .map(|feature| {
                                        BoundFeature::new(
                                            requirement.adapter_id().clone(),
                                            version,
                                            feature,
                                        )
                                    })
                                    .collect(),
                            )
                        } else {
                            SessionError::Adapter(error)
                        }
                    })
            })
            .collect::<Result<_, _>>()?;

        for requested_feature in &requested_features {
            if !bindings
                .iter()
                .any(|binding| binding.features.contains(requested_feature))
            {
                return Err(SessionError::RequestedFeatureUnavailable(
                    *requested_feature,
                ));
            }
        }

        let activation = if let Some(publication) = publication {
            self.host.activate_runtime(&target, &bindings, publication)
        } else {
            self.host.activate(&target, &bindings)
        }
        .map_err(SessionError::Host)?;
        if let Some(conflicting_feature) = activation
            .acknowledged_features
            .intersection(&activation.failed_features)
            .next()
        {
            return Err(SessionError::ConflictingHostReport(
                conflicting_feature.clone(),
            ));
        }
        let features: BTreeMap<BoundFeature, FeaturePhase> = bindings
            .iter()
            .flat_map(|binding| {
                binding.features.iter().map(|feature| {
                    let bound_feature = BoundFeature::from_binding(binding, *feature);
                    let phase = if activation.acknowledged_features.contains(&bound_feature) {
                        FeaturePhase::Active
                    } else if activation.failed_features.contains(&bound_feature) {
                        FeaturePhase::Failed
                    } else {
                        FeaturePhase::Starting
                    };
                    (bound_feature, phase)
                })
            })
            .collect();
        let next_session_id = self
            .last_session_id
            .checked_add(1)
            .ok_or(SessionError::SessionIdExhausted)?;
        let status = SessionStatus {
            session_id: SessionId::new(next_session_id),
            target_instance_id: target.id().clone(),
            diagnostics: Vec::new(),
            generations: features
                .keys()
                .cloned()
                .map(|feature| (feature, GenerationPhase::Unreported))
                .collect(),
            features,
        };

        self.last_session_id = next_session_id;
        self.sessions.insert(
            status.session_id,
            SessionRecord {
                target,
                bindings,
                controller_loss_policy: recipe.controller_loss_policy,
                status: status.clone(),
            },
        );

        Ok(status)
    }

    pub fn update(
        &mut self,
        session_id: SessionId,
        generation: Generation,
    ) -> Result<SessionStatus, SessionError> {
        self.update_inner(session_id, generation, None)
    }

    pub fn update_with_runtime(
        &mut self,
        session_id: SessionId,
        publication: &RuntimePublication,
    ) -> Result<SessionStatus, SessionError> {
        self.update_inner(session_id, publication.generation(), Some(publication))
    }

    pub fn control_capture(
        &mut self,
        session_id: SessionId,
        paused: bool,
    ) -> Result<(), SessionError> {
        let record = self
            .sessions
            .get(&session_id)
            .ok_or(SessionError::SessionNotFound(session_id))?;
        let target = record.target.clone();
        self.host
            .control_capture(session_id, &target, paused)
            .map_err(SessionError::Host)
    }

    pub fn control_runtime_diagnostics(
        &mut self,
        session_id: SessionId,
        enabled: bool,
    ) -> Result<(), SessionError> {
        let record = self
            .sessions
            .get(&session_id)
            .ok_or(SessionError::SessionNotFound(session_id))?;
        let target = record.target.clone();
        self.host
            .control_runtime_diagnostics(session_id, &target, enabled)
            .map_err(SessionError::Host)
    }

    pub fn query_runtime_diagnostics(
        &mut self,
        session_id: SessionId,
    ) -> Result<RuntimeTraceBatch, SessionError> {
        let record = self
            .sessions
            .get(&session_id)
            .ok_or(SessionError::SessionNotFound(session_id))?;
        let target = record.target.clone();
        self.host
            .query_runtime_diagnostics(session_id, &target)
            .map_err(SessionError::Host)
    }

    fn update_inner(
        &mut self,
        session_id: SessionId,
        generation: Generation,
        publication: Option<&RuntimePublication>,
    ) -> Result<SessionStatus, SessionError> {
        let record = self
            .sessions
            .get(&session_id)
            .ok_or(SessionError::SessionNotFound(session_id))?;
        let target = record.target.clone();
        let bindings = record.bindings.clone();
        let report = if let Some(publication) = publication {
            self.host
                .update_runtime(session_id, &target, &bindings, publication)
        } else {
            self.host.update(session_id, &target, &bindings, generation)
        }
        .map_err(SessionError::Host)?;
        let record = self
            .sessions
            .get_mut(&session_id)
            .ok_or(SessionError::SessionNotFound(session_id))?;

        for binding in &record.bindings {
            for feature in &binding.features {
                let bound_feature = BoundFeature::from_binding(binding, *feature);
                let previous_applied = record
                    .status
                    .generations
                    .get(&bound_feature)
                    .copied()
                    .unwrap_or(GenerationPhase::Unreported)
                    .applied();
                let acknowledged = match &binding.host {
                    AdapterHostBinding::TargetProcess { .. } => report.target_runtime_ack,
                    AdapterHostBinding::IsolatedWorker { .. } => {
                        report.isolated_feature_acks.get(&bound_feature).copied()
                    }
                };
                let phase = match acknowledged {
                    Some(acknowledged) if acknowledged == generation => {
                        GenerationPhase::Applied(generation)
                    }
                    Some(acknowledged) => GenerationPhase::Mismatch {
                        desired: generation,
                        acknowledged,
                        applied: previous_applied,
                    },
                    None => GenerationPhase::Updating {
                        desired: generation,
                        applied: previous_applied,
                    },
                };
                record.status.generations.insert(bound_feature, phase);
            }
        }

        Ok(record.status.clone())
    }

    pub fn stop(&mut self, session_id: SessionId) -> Result<SessionStatus, SessionError> {
        let record = self
            .sessions
            .get(&session_id)
            .ok_or(SessionError::SessionNotFound(session_id))?;
        let target = record.target.clone();
        let bindings = record.bindings.clone();
        let plan: Vec<_> = bindings
            .iter()
            .map(|binding| {
                let mode = match binding.apply_model {
                    ApplyModel::InlineRender => DeactivationMode::PassThrough,
                    ApplyModel::RetainedObject => DeactivationMode::RestoreOriginal,
                    ApplyModel::ExternalProtocol => DeactivationMode::StopWriteback,
                    ApplyModel::ObserveOnly => DeactivationMode::StopObserving,
                };
                AdapterDeactivation::new(
                    BoundAdapter::new(binding.adapter_id.clone(), binding.version),
                    mode,
                )
            })
            .collect();
        let report = self
            .host
            .deactivate(session_id, &target, &bindings, &plan)
            .map_err(SessionError::Host)?;
        if let Some(conflicting_adapter) = report
            .completed_adapters
            .intersection(&report.failed_adapters)
            .next()
        {
            return Err(SessionError::ConflictingHostDeactivation(
                conflicting_adapter.clone(),
            ));
        }
        let record = self
            .sessions
            .get_mut(&session_id)
            .ok_or(SessionError::SessionNotFound(session_id))?;

        for (bound_feature, phase) in &mut record.status.features {
            let adapter =
                BoundAdapter::new(bound_feature.adapter_id.clone(), bound_feature.version);
            *phase = if report.completed_adapters.contains(&adapter) {
                FeaturePhase::Ready
            } else if report.failed_adapters.contains(&adapter) {
                FeaturePhase::Failed
            } else {
                FeaturePhase::Degraded
            };
        }

        Ok(record.status.clone())
    }

    pub fn status(&mut self, session_id: SessionId) -> Result<SessionStatus, SessionError> {
        let record = self
            .sessions
            .get(&session_id)
            .ok_or(SessionError::SessionNotFound(session_id))?;
        let target = record.target.clone();
        let bindings = record.bindings.clone();
        let controller_loss_policy = record.controller_loss_policy;
        if self.target_lifecycle.health(&target) == TargetHealth::Exited {
            let release = self.host.release(session_id, &target, &bindings);
            self.sessions.remove(&session_id);
            return match release {
                Ok(()) => Err(SessionError::TargetExited(session_id)),
                Err(failure) => Err(SessionError::TargetReleaseFailed {
                    session_id,
                    failure,
                }),
            };
        }
        let controller_health = self.controller.health(&target);
        let host_health = self
            .host
            .health(session_id, &target, &bindings)
            .map_err(SessionError::Host)?;
        let invalidated_adapters: BTreeSet<_> = bindings
            .iter()
            .filter_map(|binding| {
                let requirement = AdapterRequirement::new(
                    binding.adapter_id.clone(),
                    AdapterVersionRequirement::Exact(binding.version),
                    binding.features.iter().copied(),
                );
                self.registry
                    .resolve(&requirement, target.facts())
                    .err()
                    .map(|_| BoundAdapter::new(binding.adapter_id.clone(), binding.version))
            })
            .collect();
        let record = self
            .sessions
            .get_mut(&session_id)
            .ok_or(SessionError::SessionNotFound(session_id))?;

        match (controller_health, controller_loss_policy) {
            (ControllerHealth::Lost, ControllerLossPolicy::Continue)
            | (ControllerHealth::Available, _) => {}
            (ControllerHealth::Lost, ControllerLossPolicy::Degrade) => {
                for phase in record.status.features.values_mut() {
                    if *phase != FeaturePhase::Failed {
                        *phase = FeaturePhase::Degraded;
                    }
                }
            }
        }
        for (bound_feature, phase) in &mut record.status.features {
            let adapter =
                BoundAdapter::new(bound_feature.adapter_id.clone(), bound_feature.version);
            if (host_health.failed_adapters.contains(&adapter)
                || invalidated_adapters.contains(&adapter))
                && *phase != FeaturePhase::Failed
            {
                *phase = FeaturePhase::Degraded;
            }
        }
        record.status.diagnostics = host_health.diagnostics;

        Ok(record.status.clone())
    }
}

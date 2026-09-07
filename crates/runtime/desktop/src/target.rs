use super::*;
use crate::acquisition::{map_acquisition_protocol_error, AcquisitionExecutor};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeTarget {
    pub(super) id: u64,
    pub(super) display_name: Box<str>,
}

impl RuntimeTarget {
    #[must_use]
    pub const fn id(&self) -> u64 {
        self.id
    }

    #[must_use]
    pub fn display_name(&self) -> &str {
        &self.display_name
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TargetRecord {
    view: RuntimeTarget,
    controller_id: OpaqueTargetId,
    facts: TargetFacts,
}
pub type WindowsDesktopRuntime = DesktopRuntime<
    glyphshift_protocol::ArchitectureControllerTransport<ProcessControllerTransport>,
>;

pub(super) trait ManagedRuntime: Send {
    fn application_id(&self) -> &str;
    fn targets(&self) -> Vec<RuntimeTarget>;
    fn supported_features(&self) -> BTreeSet<Feature>;
    fn active_features(&self) -> BTreeSet<Feature>;
    fn active_target_id(&self) -> Option<u64>;
    fn active_target_count(&self) -> usize {
        usize::from(self.active_target_id().is_some())
    }
    fn failed_target_count(&self) -> usize {
        0
    }
    fn applied_generation(&self) -> Option<Generation>;
    fn start(
        &mut self,
        target_id: u64,
        requested_features: &BTreeSet<Feature>,
    ) -> Result<(), DesktopRuntimeError>;
    fn start_capture(
        &mut self,
        target_ids: &[u64],
        requested_features: &BTreeSet<Feature>,
        capture: CaptureConfiguration,
    ) -> Result<(), DesktopRuntimeError>;
    fn acquire(
        &mut self,
        target_id: u64,
        adapter_id: &str,
        selection: InteractiveSelection,
        cancellation: &DesktopAcquisitionCancellation,
    ) -> Result<AcquisitionResult, DesktopAcquisitionError>;
    fn publish(
        &mut self,
        publication: glyphshift_runtime_contract::RuntimePublication,
    ) -> Result<(), DesktopRuntimeError>;
    fn control_capture(&mut self, paused: bool) -> Result<(), DesktopRuntimeError>;
    fn control_runtime_diagnostics(&mut self, enabled: bool) -> Result<(), DesktopRuntimeError>;
    fn query_runtime_diagnostics(&mut self) -> Result<RuntimeTraceBatch, DesktopRuntimeError>;
    fn stop(&mut self) -> Result<(), DesktopRuntimeError>;
    fn abandon(&mut self) {}

    fn is_active(&self) -> bool {
        self.active_target_id().is_some() && !self.active_features().is_empty()
    }
}

impl ManagedRuntime for WindowsDesktopRuntime {
    fn application_id(&self) -> &str {
        &self.application_id
    }

    fn targets(&self) -> Vec<RuntimeTarget> {
        self.targets
            .iter()
            .map(|target| target.view.clone())
            .collect()
    }

    fn supported_features(&self) -> BTreeSet<Feature> {
        self.supported_features.clone()
    }

    fn active_features(&self) -> BTreeSet<Feature> {
        self.active_features.clone()
    }

    fn active_target_id(&self) -> Option<u64> {
        DesktopRuntime::active_target_id(self)
    }

    fn active_target_count(&self) -> usize {
        match self.phase.as_ref() {
            Some(RuntimePhase::Active { sessions, .. }) => sessions.len(),
            _ => 0,
        }
    }

    fn failed_target_count(&self) -> usize {
        match self.phase.as_ref() {
            Some(RuntimePhase::Active { failures, .. }) => failures.len(),
            _ => 0,
        }
    }

    fn applied_generation(&self) -> Option<Generation> {
        self.is_active().then(|| self.publication.generation())
    }

    fn start(
        &mut self,
        target_id: u64,
        requested_features: &BTreeSet<Feature>,
    ) -> Result<(), DesktopRuntimeError> {
        DesktopRuntime::start(self, target_id, requested_features.iter().copied())
    }

    fn start_capture(
        &mut self,
        target_ids: &[u64],
        requested_features: &BTreeSet<Feature>,
        capture: CaptureConfiguration,
    ) -> Result<(), DesktopRuntimeError> {
        DesktopRuntime::start_capture(
            self,
            target_ids.iter().copied(),
            requested_features.iter().copied(),
            capture,
        )
    }

    fn acquire(
        &mut self,
        target_id: u64,
        adapter_id: &str,
        selection: InteractiveSelection,
        cancellation: &DesktopAcquisitionCancellation,
    ) -> Result<AcquisitionResult, DesktopAcquisitionError> {
        DesktopRuntime::acquire(self, target_id, adapter_id, selection, cancellation)
    }

    fn publish(
        &mut self,
        publication: glyphshift_runtime_contract::RuntimePublication,
    ) -> Result<(), DesktopRuntimeError> {
        DesktopRuntime::publish(self, publication)
    }

    fn control_capture(&mut self, paused: bool) -> Result<(), DesktopRuntimeError> {
        DesktopRuntime::control_capture(self, paused)
    }

    fn control_runtime_diagnostics(&mut self, enabled: bool) -> Result<(), DesktopRuntimeError> {
        DesktopRuntime::control_runtime_diagnostics(self, enabled)
    }

    fn query_runtime_diagnostics(&mut self) -> Result<RuntimeTraceBatch, DesktopRuntimeError> {
        DesktopRuntime::query_runtime_diagnostics(self)
    }

    fn stop(&mut self) -> Result<(), DesktopRuntimeError> {
        DesktopRuntime::stop(self)
    }

    fn abandon(&mut self) {
        DesktopRuntime::abandon(self);
    }
}
enum RuntimePhase<T> {
    Discovered(ControllerConnection<T>),
    Active {
        manager: SessionManager,
        sessions: BTreeMap<u64, SessionId>,
        failures: BTreeMap<u64, DesktopRuntimeError>,
        capture_owner: Option<FileCaptureSink>,
    },
}

/// One selected application's runtime session. Constructed by [`RuntimeBundle::discover`].
pub struct DesktopRuntime<T> {
    application_id: Box<str>,
    supported_features: BTreeSet<Feature>,
    publication: glyphshift_runtime_contract::RuntimePublication,
    registry: AdapterRegistry,
    artifacts: TargetArtifactCatalog,
    worker_artifacts: WorkerArtifactCatalog,
    acquisition: Box<dyn AcquisitionExecutor>,
    isolated_adapter_ids: BTreeSet<AdapterId>,
    targets: Vec<TargetRecord>,
    active_features: BTreeSet<Feature>,
    phase: Option<RuntimePhase<T>>,
}

impl<T: ControllerTransport + Send + 'static> DesktopRuntime<T> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn connect(
        transport: T,
        application_id: Box<str>,
        requirements: Vec<AdapterRequirement>,
        publication: glyphshift_runtime_contract::RuntimePublication,
        registry: AdapterRegistry,
        artifacts: TargetArtifactCatalog,
        worker_artifacts: WorkerArtifactCatalog,
        acquisition: Box<dyn AcquisitionExecutor>,
        isolated_adapter_ids: BTreeSet<AdapterId>,
        protocol: ProtocolVersion,
        nonce: ControllerNonce,
        ledger: &mut NonceLedger,
    ) -> Result<Self, DesktopRuntimeError> {
        let mut connection = ControllerConnection::connect(
            transport,
            ExtensionId::new(application_id.clone()),
            protocol,
            nonce,
            ledger,
        )
        .map_err(|_| DesktopRuntimeError::ProtocolRejected)?;
        connection.authorize_capabilities(requirements.iter().cloned());
        let inventory = connection
            .inventory()
            .map_err(|_| DesktopRuntimeError::ControllerUnavailable)?;
        let targets = inventory
            .targets()
            .iter()
            .map(|target| TargetRecord {
                view: RuntimeTarget {
                    id: target.id().as_u64(),
                    display_name: target.display_name().into(),
                },
                controller_id: target.id(),
                facts: target.facts().clone(),
            })
            .collect();
        let supported_features = requirements
            .iter()
            .flat_map(AdapterRequirement::features)
            .collect();
        Ok(Self {
            application_id,
            supported_features,
            publication,
            registry,
            artifacts,
            worker_artifacts,
            acquisition,
            isolated_adapter_ids,
            targets,
            active_features: BTreeSet::new(),
            phase: Some(RuntimePhase::Discovered(connection)),
        })
    }

    #[must_use]
    pub fn application_id(&self) -> &str {
        &self.application_id
    }

    pub fn targets(&self) -> impl Iterator<Item = &RuntimeTarget> {
        self.targets.iter().map(|target| &target.view)
    }

    pub fn acquire_point(
        &mut self,
        target_id: u64,
        adapter_id: &str,
        point: DesktopPoint,
        cancellation: &DesktopAcquisitionCancellation,
    ) -> Result<AcquisitionResult, DesktopAcquisitionError> {
        self.acquire(
            target_id,
            adapter_id,
            InteractiveSelection::Point(point),
            cancellation,
        )
    }

    pub fn acquire_region(
        &mut self,
        target_id: u64,
        adapter_id: &str,
        region: DesktopRect,
        cancellation: &DesktopAcquisitionCancellation,
    ) -> Result<AcquisitionResult, DesktopAcquisitionError> {
        self.acquire(
            target_id,
            adapter_id,
            InteractiveSelection::Region(region),
            cancellation,
        )
    }

    fn acquire(
        &mut self,
        target_id: u64,
        adapter_id: &str,
        selection: InteractiveSelection,
        cancellation: &DesktopAcquisitionCancellation,
    ) -> Result<AcquisitionResult, DesktopAcquisitionError> {
        if cancellation.is_cancelled() {
            return Err(DesktopAcquisitionError::Cancelled);
        }
        let target = self
            .targets
            .iter()
            .find(|target| target.view.id == target_id)
            .cloned()
            .ok_or(DesktopAcquisitionError::UnknownTarget)?;
        let Some(RuntimePhase::Discovered(connection)) = self.phase.as_mut() else {
            return Err(DesktopAcquisitionError::InvalidState);
        };
        let grant = connection
            .authorize_worker_target(target.controller_id)
            .map_err(map_acquisition_protocol_error)?;
        let authorized_target = AuthorizedTarget::new(format!("target-{}", target.view.id))
            .map_err(|_| DesktopAcquisitionError::TargetUnavailable)?;
        self.acquisition.acquire(
            adapter_id,
            authorized_target,
            grant,
            selection,
            cancellation,
        )
    }

    #[must_use]
    pub fn supports(&self, feature: Feature) -> bool {
        self.supported_features.contains(&feature)
    }

    #[must_use]
    pub fn is_feature_active(&self, feature: Feature) -> bool {
        self.is_active() && self.active_features.contains(&feature)
    }

    #[must_use]
    pub fn is_active(&self) -> bool {
        matches!(self.phase, Some(RuntimePhase::Active { .. }))
    }

    #[must_use]
    pub fn active_target_id(&self) -> Option<u64> {
        match self.phase.as_ref() {
            Some(RuntimePhase::Active { sessions, .. }) => sessions.keys().next().copied(),
            _ => None,
        }
    }

    #[must_use]
    pub fn active_target_count(&self) -> usize {
        match self.phase.as_ref() {
            Some(RuntimePhase::Active { sessions, .. }) => sessions.len(),
            _ => 0,
        }
    }

    #[must_use]
    pub fn failed_target_count(&self) -> usize {
        match self.phase.as_ref() {
            Some(RuntimePhase::Active { failures, .. }) => failures.len(),
            _ => 0,
        }
    }

    pub fn start(
        &mut self,
        target_id: u64,
        requested_features: impl IntoIterator<Item = Feature>,
    ) -> Result<(), DesktopRuntimeError> {
        self.start_inner([target_id], requested_features, None)
    }

    pub fn start_capture(
        &mut self,
        target_ids: impl IntoIterator<Item = u64>,
        requested_features: impl IntoIterator<Item = Feature>,
        capture: CaptureConfiguration,
    ) -> Result<(), DesktopRuntimeError> {
        self.start_inner(target_ids, requested_features, Some(capture))
    }

    fn start_inner(
        &mut self,
        target_ids: impl IntoIterator<Item = u64>,
        requested_features: impl IntoIterator<Item = Feature>,
        capture: Option<CaptureConfiguration>,
    ) -> Result<(), DesktopRuntimeError> {
        let requested_features = requested_features.into_iter().collect::<Vec<_>>();
        let capture_requested = capture.is_some();
        if requested_features.is_empty()
            || requested_features
                .iter()
                .any(|feature| !self.supported_features.contains(feature))
        {
            return Err(DesktopRuntimeError::SessionRejected);
        }
        let target_ids = target_ids.into_iter().collect::<BTreeSet<_>>();
        if target_ids.is_empty() {
            return Err(DesktopRuntimeError::UnknownTarget);
        }
        let targets = target_ids
            .iter()
            .map(|target_id| {
                self.targets
                    .iter()
                    .find(|target| target.view.id == *target_id)
                    .cloned()
                    .ok_or(DesktopRuntimeError::UnknownTarget)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut connection = match self.phase.take() {
            Some(RuntimePhase::Discovered(connection)) => connection,
            phase => {
                self.phase = phase;
                return Err(DesktopRuntimeError::InvalidState);
            }
        };
        let mut prepared = Vec::new();
        let mut runtime_targets = Vec::new();
        let mut failures = BTreeMap::new();
        for target in targets {
            let recipe = match connection
                .prepare(target.controller_id, requested_features.iter().copied())
            {
                Ok(recipe) => recipe,
                Err(error) => {
                    failures.insert(target.view.id, map_protocol_error(error));
                    continue;
                }
            };
            let target_instance_id = TargetInstanceId::new(format!("target-{}", target.view.id));
            let requires_isolated_worker = recipe
                .requirements()
                .iter()
                .any(|requirement| self.isolated_adapter_ids.contains(requirement.adapter_id()));
            if requires_isolated_worker && !capture_requested {
                failures.insert(target.view.id, DesktopRuntimeError::SessionRejected);
                continue;
            }
            let worker_grant = if requires_isolated_worker {
                let grant = match connection.authorize_worker_target(target.controller_id) {
                    Ok(grant) => grant,
                    Err(error) => {
                        failures.insert(target.view.id, map_protocol_error(error));
                        continue;
                    }
                };
                Some(WorkerTargetGrant {
                    platform: grant.platform().into(),
                    payload: grant.payload().into(),
                })
            } else {
                None
            };
            prepared.push((target_instance_id.clone(), recipe));
            runtime_targets.push((
                target.view.id,
                target_instance_id.clone(),
                target.controller_id,
                worker_grant,
                TargetInstance::new(target_instance_id, target.facts),
            ));
        }
        if runtime_targets.is_empty() {
            self.phase = Some(RuntimePhase::Discovered(connection));
            return Err(failures
                .into_values()
                .next()
                .unwrap_or(DesktopRuntimeError::SessionRejected));
        }
        let capture_owner = match capture.map(FileCaptureSink::start).transpose() {
            Ok(owner) => owner,
            Err(_) => {
                self.phase = Some(RuntimePhase::Discovered(connection));
                return Err(DesktopRuntimeError::SessionRejected);
            }
        };
        let mut host = TargetProcessHost::new(connection, self.artifacts.clone());
        if let Some(owner) = capture_owner.as_ref() {
            host = host.with_capture_ingress(owner.ingress());
        }
        for (_, target_instance_id, controller_id, _, _) in &runtime_targets {
            host.register_target(target_instance_id.clone(), *controller_id);
        }
        let mut hybrid_host = HybridAdapterHost::new(host);
        if runtime_targets
            .iter()
            .any(|(_, _, _, grant, _)| grant.is_some())
        {
            let owner = capture_owner
                .as_ref()
                .ok_or(DesktopRuntimeError::SessionRejected)?;
            let mut isolated_worker = IsolatedWorkerHost::new(
                self.worker_artifacts.clone(),
                owner.ingress(),
                ISOLATED_WORKER_TIMEOUT,
            );
            for (_, target_instance_id, _, grant, _) in &runtime_targets {
                if let Some(grant) = grant {
                    isolated_worker.register_target(target_instance_id.clone(), grant.clone());
                }
            }
            hybrid_host = hybrid_host.with_isolated_worker(isolated_worker);
        }
        let mut manager = SessionManager::new(
            self.registry.clone(),
            PreparedController::new(prepared),
            hybrid_host,
            RunningTarget,
        );
        let mut active_features = BTreeSet::new();
        let mut sessions = BTreeMap::new();
        for (target_id, _, _, _, target_instance) in runtime_targets {
            let status = match manager.start_with_runtime(
                target_instance,
                requested_features.clone(),
                &self.publication,
            ) {
                Ok(status) => status,
                Err(error) => {
                    failures.insert(target_id, map_session_runtime_error(error));
                    continue;
                }
            };
            active_features.extend(status.active_features());
            sessions.insert(target_id, status.session_id());
        }
        if sessions.is_empty() {
            if let Some(owner) = capture_owner {
                let _ = owner.finish();
            }
            return Err(failures
                .into_values()
                .next()
                .unwrap_or(DesktopRuntimeError::SessionRejected));
        }
        self.phase = Some(RuntimePhase::Active {
            manager,
            sessions,
            failures,
            capture_owner,
        });
        self.active_features = active_features;
        Ok(())
    }

    pub fn publish(
        &mut self,
        publication: glyphshift_runtime_contract::RuntimePublication,
    ) -> Result<(), DesktopRuntimeError> {
        let Some(RuntimePhase::Active {
            manager, sessions, ..
        }) = self.phase.as_mut()
        else {
            return Err(DesktopRuntimeError::InvalidState);
        };
        for session_id in sessions.values().copied() {
            manager
                .update_with_runtime(session_id, &publication)
                .map_err(|_| DesktopRuntimeError::SessionRejected)?;
        }
        self.publication = publication;
        Ok(())
    }

    pub fn control_capture(&mut self, paused: bool) -> Result<(), DesktopRuntimeError> {
        let Some(RuntimePhase::Active {
            manager,
            sessions,
            capture_owner,
            ..
        }) = self.phase.as_mut()
        else {
            return Err(DesktopRuntimeError::InvalidState);
        };
        let capture_owner = capture_owner
            .as_ref()
            .ok_or(DesktopRuntimeError::InvalidState)?;
        capture_owner.set_paused(paused);
        for session_id in sessions.values().copied() {
            if manager.control_capture(session_id, paused).is_err() {
                if !paused {
                    capture_owner.set_paused(true);
                }
                return Err(DesktopRuntimeError::SessionRejected);
            }
        }
        Ok(())
    }

    pub fn control_runtime_diagnostics(
        &mut self,
        enabled: bool,
    ) -> Result<(), DesktopRuntimeError> {
        let Some(RuntimePhase::Active {
            manager, sessions, ..
        }) = self.phase.as_mut()
        else {
            return Err(DesktopRuntimeError::InvalidState);
        };
        for session_id in sessions.values().copied() {
            manager
                .control_runtime_diagnostics(session_id, enabled)
                .map_err(|_| DesktopRuntimeError::SessionRejected)?;
        }
        Ok(())
    }

    pub fn query_runtime_diagnostics(&mut self) -> Result<RuntimeTraceBatch, DesktopRuntimeError> {
        let Some(RuntimePhase::Active {
            manager, sessions, ..
        }) = self.phase.as_mut()
        else {
            return Err(DesktopRuntimeError::InvalidState);
        };
        let session_id = sessions
            .values()
            .next()
            .copied()
            .ok_or(DesktopRuntimeError::InvalidState)?;
        manager
            .query_runtime_diagnostics(session_id)
            .map_err(|_| DesktopRuntimeError::SessionRejected)
    }

    pub fn stop(&mut self) -> Result<(), DesktopRuntimeError> {
        let Some(RuntimePhase::Active {
            manager,
            sessions,
            capture_owner,
            ..
        }) = self.phase.as_mut()
        else {
            return Err(DesktopRuntimeError::InvalidState);
        };
        let active_sessions = sessions
            .iter()
            .map(|(target_id, session_id)| (*target_id, *session_id))
            .collect::<Vec<_>>();
        for (target_id, session_id) in active_sessions {
            manager
                .stop(session_id)
                .map_err(|_| DesktopRuntimeError::SessionRejected)?;
            sessions.remove(&target_id);
        }
        if let Some(owner) = capture_owner.take() {
            owner
                .finish()
                .map_err(|_| DesktopRuntimeError::SessionRejected)?;
        }
        self.active_features.clear();
        self.phase = None;
        Ok(())
    }

    fn abandon(&mut self) {
        self.active_features.clear();
        self.phase = None;
    }
}

impl<T> Drop for DesktopRuntime<T> {
    fn drop(&mut self) {
        if let Some(RuntimePhase::Active {
            manager,
            sessions,
            capture_owner,
            ..
        }) = self.phase.as_mut()
        {
            for session_id in sessions.values().copied() {
                let _ = manager.stop(session_id);
            }
            if let Some(owner) = capture_owner.take() {
                let _ = owner.finish();
            }
        }
    }
}

struct PreparedController {
    recipes: BTreeMap<TargetInstanceId, SessionRecipe>,
}

impl PreparedController {
    fn new(prepared: impl IntoIterator<Item = (TargetInstanceId, PreparedRecipe)>) -> Self {
        Self {
            recipes: prepared
                .into_iter()
                .map(|(target, prepared)| {
                    let loss_policy = match prepared.controller_loss_policy() {
                        RecipeControllerLossPolicy::Continue => ControllerLossPolicy::Continue,
                        RecipeControllerLossPolicy::Degrade => ControllerLossPolicy::Degrade,
                    };
                    (
                        target,
                        SessionRecipe::new(prepared.requirements().iter().cloned(), loss_policy),
                    )
                })
                .collect(),
        }
    }
}

impl ControllerRecipePort for PreparedController {
    fn prepare(
        &mut self,
        target: &TargetInstance,
        _requested_features: &BTreeSet<Feature>,
    ) -> Result<SessionRecipe, ControllerFailure> {
        self.recipes
            .get(target.id())
            .cloned()
            .ok_or(ControllerFailure::Unavailable)
    }

    fn health(&mut self, _target: &TargetInstance) -> ControllerHealth {
        ControllerHealth::Available
    }
}

struct RunningTarget;

impl TargetLifecyclePort for RunningTarget {
    fn health(&mut self, _target: &TargetInstance) -> TargetHealth {
        TargetHealth::Running
    }
}

fn map_protocol_error(error: ControllerProtocolError) -> DesktopRuntimeError {
    let ControllerProtocolError::Transport(glyphshift_protocol::TransportFailure::Rejected(reason)) =
        error
    else {
        return DesktopRuntimeError::ProtocolRejected;
    };
    DesktopRuntimeError::ActivationRejected(map_controller_rejection(reason))
}

fn map_session_runtime_error(error: SessionError) -> DesktopRuntimeError {
    match error {
        SessionError::Host(HostFailure::OperationRejected(reason)) => {
            DesktopRuntimeError::ActivationRejected(reason)
        }
        _ => DesktopRuntimeError::SessionRejected,
    }
}

fn map_controller_rejection(
    reason: glyphshift_protocol::ControllerRejection,
) -> HostOperationFailure {
    use glyphshift_protocol::ControllerRejection;

    match reason {
        ControllerRejection::TargetProcessUnavailable => {
            HostOperationFailure::TargetProcessUnavailable
        }
        ControllerRejection::RemoteMemoryUnavailable => {
            HostOperationFailure::RemoteMemoryUnavailable
        }
        ControllerRejection::RuntimeModuleUnavailable => {
            HostOperationFailure::RuntimeModuleUnavailable
        }
        ControllerRejection::RuntimeExportUnavailable => {
            HostOperationFailure::RuntimeExportUnavailable
        }
        ControllerRejection::RemoteThreadUnavailable => {
            HostOperationFailure::RemoteThreadUnavailable
        }
        ControllerRejection::RemoteThreadTimeout => HostOperationFailure::RemoteThreadTimeout,
        ControllerRejection::TargetRuntimeRejected(status) => {
            HostOperationFailure::TargetRuntimeRejected(status)
        }
        ControllerRejection::Unknown => HostOperationFailure::ControllerRejected,
    }
}

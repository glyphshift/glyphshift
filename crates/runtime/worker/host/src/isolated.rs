use crate::supervisor::{allocate_producer_generation, WorkerSupervisor};
use crate::{WorkerArtifactCatalog, WorkerHealth, WorkerHostError};
use glyphshift_adapter_registry::{AdapterBinding, AdapterHostBinding};
use glyphshift_capture::{CaptureIngress, CaptureProducerConfiguration};
use glyphshift_isolated_worker_sdk::WorkerTargetGrant;
use glyphshift_runtime_contract::RuntimePublication;
use glyphshift_session::{
    AdapterDeactivation, AdapterHostPort, BoundAdapter, BoundFeature, HostActivation,
    HostDeactivation, HostFailure, HostGenerationReport, HostHealthReport, HostOperationFailure,
    SessionDiagnostic, SessionId, TargetInstance, TargetInstanceId,
};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use std::time::Duration;

/// Adapter Host for one isolated worker process per target and Adapter binding.
pub struct IsolatedWorkerHost {
    artifacts: WorkerArtifactCatalog,
    targets: BTreeMap<TargetInstanceId, WorkerTargetGrant>,
    workers: BTreeMap<TargetInstanceId, BTreeMap<BoundAdapter, WorkerSupervisor>>,
    capture_ingress: CaptureIngress,
    timeout: Duration,
    next_producer_generation: Arc<AtomicU64>,
}

impl IsolatedWorkerHost {
    #[must_use]
    pub fn new(
        artifacts: WorkerArtifactCatalog,
        capture_ingress: CaptureIngress,
        timeout: Duration,
    ) -> Self {
        Self {
            artifacts,
            targets: BTreeMap::new(),
            workers: BTreeMap::new(),
            capture_ingress,
            timeout,
            next_producer_generation: Arc::new(AtomicU64::new(1)),
        }
    }

    pub fn register_target(
        &mut self,
        target_instance_id: TargetInstanceId,
        target_grant: WorkerTargetGrant,
    ) {
        self.targets.insert(target_instance_id, target_grant);
    }

    fn isolated_bindings(bindings: &[AdapterBinding]) -> impl Iterator<Item = &AdapterBinding> {
        bindings
            .iter()
            .filter(|binding| matches!(binding.host, AdapterHostBinding::IsolatedWorker { .. }))
    }

    fn isolated_features(bindings: &[AdapterBinding]) -> Vec<BoundFeature> {
        Self::isolated_bindings(bindings)
            .flat_map(|binding| {
                binding.features.iter().copied().map(|feature| {
                    BoundFeature::new(binding.adapter_id.clone(), binding.version, feature)
                })
            })
            .collect()
    }

    fn isolated_adapters(bindings: &[AdapterBinding]) -> Vec<BoundAdapter> {
        Self::isolated_bindings(bindings)
            .map(|binding| BoundAdapter::new(binding.adapter_id.clone(), binding.version))
            .collect()
    }

    fn activate_generation(
        &mut self,
        target: &TargetInstance,
        bindings: &[AdapterBinding],
        publication_generation: u64,
    ) -> Result<HostActivation, HostFailure> {
        let target_grant = self
            .targets
            .get(target.id())
            .cloned()
            .ok_or(HostFailure::Unavailable)?;
        if self.workers.contains_key(target.id()) {
            return Err(HostFailure::Unavailable);
        }
        let mut started = BTreeMap::new();
        for binding in Self::isolated_bindings(bindings) {
            let AdapterHostBinding::IsolatedWorker { executable } = &binding.host else {
                continue;
            };
            let artifact = self
                .artifacts
                .artifact(executable)
                .ok_or(HostFailure::Unavailable)?;
            let producer_generation = allocate_producer_generation(&self.next_producer_generation)
                .map_err(|_| HostFailure::Unavailable)?;
            let producer = CaptureProducerConfiguration::new(
                glyphshift_capture::CaptureProducerId::new(format!(
                    "isolated-{producer_generation}"
                ))
                .map_err(|_| HostFailure::Unavailable)?,
                producer_generation,
            )
            .map_err(|_| HostFailure::Unavailable)?;
            let adapter = BoundAdapter::new(binding.adapter_id.clone(), binding.version);
            let supervisor = WorkerSupervisor::start(
                artifact,
                self.timeout,
                &binding.adapter_id,
                target_grant.clone(),
                producer,
                publication_generation,
                self.capture_ingress.clone(),
                Arc::clone(&self.next_producer_generation),
            )
            .map_err(map_worker_error)?;
            started.insert(adapter, supervisor);
        }
        self.workers.insert(target.id().clone(), started);
        Ok(HostActivation::connected(Self::isolated_features(bindings)))
    }
}

pub(super) fn map_worker_error(error: WorkerHostError) -> HostFailure {
    match error {
        WorkerHostError::HandshakeRejected | WorkerHostError::MalformedMessage => {
            HostFailure::HandshakeRejected
        }
        WorkerHostError::WorkerRejected(code) if code.as_ref() == "uia_permission_denied" => {
            HostFailure::OperationRejected(HostOperationFailure::IsolatedWorkerPermissionDenied)
        }
        WorkerHostError::Timeout => {
            HostFailure::OperationRejected(HostOperationFailure::IsolatedWorkerTimeout)
        }
        _ => HostFailure::Unavailable,
    }
}

impl AdapterHostPort for IsolatedWorkerHost {
    fn activate(
        &mut self,
        target: &TargetInstance,
        bindings: &[AdapterBinding],
    ) -> Result<HostActivation, HostFailure> {
        self.activate_generation(target, bindings, 1)
    }

    fn activate_runtime(
        &mut self,
        target: &TargetInstance,
        bindings: &[AdapterBinding],
        publication: &RuntimePublication,
    ) -> Result<HostActivation, HostFailure> {
        self.activate_generation(target, bindings, publication.generation().value())
    }

    fn update(
        &mut self,
        _session_id: SessionId,
        target: &TargetInstance,
        bindings: &[AdapterBinding],
        generation: glyphshift_domain::Generation,
    ) -> Result<HostGenerationReport, HostFailure> {
        let workers = self
            .workers
            .get(target.id())
            .ok_or(HostFailure::Unavailable)?;
        let mut acknowledgements = Vec::new();
        for binding in Self::isolated_bindings(bindings) {
            let adapter = BoundAdapter::new(binding.adapter_id.clone(), binding.version);
            let acknowledged = workers
                .get(&adapter)
                .ok_or(HostFailure::Unavailable)?
                .update_generation(generation.value())
                .map_err(map_worker_error)?;
            let acknowledged = glyphshift_domain::Generation::new(acknowledged);
            acknowledgements.extend(binding.features.iter().copied().map(|feature| {
                (
                    BoundFeature::new(binding.adapter_id.clone(), binding.version, feature),
                    acknowledged,
                )
            }));
        }
        Ok(HostGenerationReport::isolated(acknowledgements))
    }

    fn control_capture(
        &mut self,
        _session_id: SessionId,
        target: &TargetInstance,
        paused: bool,
    ) -> Result<(), HostFailure> {
        for supervisor in self
            .workers
            .get(target.id())
            .ok_or(HostFailure::Unavailable)?
            .values()
        {
            supervisor.set_paused(paused).map_err(map_worker_error)?;
        }
        Ok(())
    }

    fn health(
        &mut self,
        _session_id: SessionId,
        target: &TargetInstance,
        bindings: &[AdapterBinding],
    ) -> Result<HostHealthReport, HostFailure> {
        let workers = self
            .workers
            .get(target.id())
            .ok_or(HostFailure::Unavailable)?;
        let mut failed = BTreeSet::new();
        let mut diagnostics = Vec::new();
        for adapter in Self::isolated_adapters(bindings) {
            match workers
                .get(&adapter)
                .and_then(|worker| worker.health().ok())
            {
                Some(report) if report.state() == WorkerHealth::Healthy => {}
                Some(report) => {
                    failed.insert(adapter);
                    diagnostics.push(SessionDiagnostic::new(
                        report.code().unwrap_or("isolated_worker_degraded"),
                    ));
                }
                None => {
                    failed.insert(adapter);
                    diagnostics.push(SessionDiagnostic::new("isolated_worker_unavailable"));
                }
            }
        }
        Ok(HostHealthReport::reported(failed, diagnostics))
    }

    fn deactivate(
        &mut self,
        _session_id: SessionId,
        target: &TargetInstance,
        bindings: &[AdapterBinding],
        _plan: &[AdapterDeactivation],
    ) -> Result<HostDeactivation, HostFailure> {
        let mut completed = BTreeSet::new();
        let mut failed = BTreeSet::new();
        if let Some(workers) = self.workers.remove(target.id()) {
            for (adapter, mut worker) in workers {
                if worker.finish().is_ok() {
                    completed.insert(adapter);
                } else {
                    failed.insert(adapter);
                }
            }
        } else {
            failed.extend(Self::isolated_adapters(bindings));
        }
        Ok(HostDeactivation::reported(completed, failed))
    }

    fn release(
        &mut self,
        _session_id: SessionId,
        target: &TargetInstance,
        _bindings: &[AdapterBinding],
    ) -> Result<(), HostFailure> {
        if let Some(workers) = self.workers.remove(target.id()) {
            for (_, mut worker) in workers {
                let _ = worker.finish();
            }
        }
        self.targets.remove(target.id());
        Ok(())
    }
}

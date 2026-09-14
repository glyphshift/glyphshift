//! Production Adapter Host for injected target-process Runtime instances.

use glyphshift_adapter_registry::{AdapterBinding, AdapterHostBinding, PackageArtifactId};
use glyphshift_capture::{
    CaptureIngress, CaptureObservationCursor, CaptureProducerConfiguration, CaptureProducerId,
};
use glyphshift_protocol::{
    ControllerConnection, ControllerHealth, ControllerProtocolError, ControllerRejection,
    ControllerRuntimeDeployment, ControllerRuntimeFontOutcome, ControllerRuntimeTextOutcome,
    ControllerRuntimeTraceStatus, ControllerTransport, OpaqueTargetId, TransportFailure,
};
use glyphshift_runtime_contract::RuntimePublication;
use glyphshift_session::{
    AdapterDeactivation, AdapterHostPort, BoundAdapter, BoundFeature, HostActivation,
    HostDeactivation, HostFailure, HostGenerationReport, HostHealthReport, HostOperationFailure,
    RuntimeFontOutcome, RuntimeTextOutcome, RuntimeTraceBatch, RuntimeTraceRecord,
    RuntimeTraceStatus, SessionId, TargetInstance, TargetInstanceId,
};
use glyphshift_target_runtime_contract::{
    NativeAdapterDeployment, TargetRuntimeDeployment, STATUS_TARGET_RUNTIME_INVALID_DEPLOYMENT,
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{sync_channel, Receiver, RecvTimeoutError, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

const CAPTURE_POLL_INTERVAL: Duration = Duration::from_millis(250);
const MAX_BATCHES_PER_DRAIN: usize = 64;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeArtifact {
    library: PathBuf,
    sha256: [u8; 32],
}

impl RuntimeArtifact {
    #[must_use]
    pub fn new(library: impl Into<PathBuf>, sha256: [u8; 32]) -> Self {
        Self {
            library: library.into(),
            sha256,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ArtifactCatalogError {
    RuntimeLibraryUnavailable,
    AdapterLibraryUnavailable(PackageArtifactId),
    DuplicateAdapterArtifact(PackageArtifactId),
}

#[derive(Clone, Debug)]
pub struct TargetArtifactCatalog {
    runtime: RuntimeArtifact,
    architecture_runtimes: BTreeMap<Box<str>, RuntimeArtifact>,
    adapters: BTreeMap<PackageArtifactId, PathBuf>,
}

impl TargetArtifactCatalog {
    pub fn new(
        runtime: RuntimeArtifact,
        adapters: impl IntoIterator<Item = (PackageArtifactId, PathBuf)>,
    ) -> Result<Self, ArtifactCatalogError> {
        if !available_library(&runtime.library) {
            return Err(ArtifactCatalogError::RuntimeLibraryUnavailable);
        }
        let mut indexed = BTreeMap::new();
        for (artifact_id, library) in adapters {
            if !available_library(&library) {
                return Err(ArtifactCatalogError::AdapterLibraryUnavailable(artifact_id));
            }
            if indexed.insert(artifact_id.clone(), library).is_some() {
                return Err(ArtifactCatalogError::DuplicateAdapterArtifact(artifact_id));
            }
        }
        Ok(Self {
            runtime,
            architecture_runtimes: BTreeMap::new(),
            adapters: indexed,
        })
    }

    pub fn with_architecture_runtimes(
        mut self,
        runtimes: impl IntoIterator<Item = (Box<str>, RuntimeArtifact)>,
    ) -> Result<Self, ArtifactCatalogError> {
        for (architecture, runtime) in runtimes {
            if !matches!(architecture.as_ref(), "x86" | "x86_64")
                || !available_library(&runtime.library)
                || self
                    .architecture_runtimes
                    .insert(architecture, runtime)
                    .is_some()
            {
                return Err(ArtifactCatalogError::RuntimeLibraryUnavailable);
            }
        }
        Ok(self)
    }
    fn runtime_for(&self, architecture: &str) -> Option<&RuntimeArtifact> {
        if self.architecture_runtimes.is_empty() {
            Some(&self.runtime)
        } else {
            self.architecture_runtimes.get(architecture)
        }
    }
}

fn available_library(path: &Path) -> bool {
    path.is_absolute() && path.is_file()
}

/// Read-only process-instance inventory shared with the owning host connection.
pub struct TargetProcessMonitor<T> {
    connection: Arc<Mutex<ControllerConnection<T>>>,
}
impl<T: ControllerTransport> TargetProcessMonitor<T> {
    pub fn running_targets(&self, targets: &[OpaqueTargetId]) -> Result<std::collections::BTreeSet<OpaqueTargetId>, ControllerProtocolError> {
        self.connection.lock().map_err(|_| ControllerProtocolError::Transport(TransportFailure::MalformedMessage))?
            .running_targets(targets)
    }
}

pub struct TargetProcessHost<T> {
    connection: Arc<Mutex<ControllerConnection<T>>>,
    artifacts: TargetArtifactCatalog,
    targets: BTreeMap<TargetInstanceId, OpaqueTargetId>,
    capture_ingress: Option<CaptureIngress>,
    capture_supervisors: BTreeMap<OpaqueTargetId, CaptureSupervisor>,
    next_capture_generation: u64,
}

impl<T> TargetProcessHost<T> {
    #[must_use]
    pub fn new(connection: ControllerConnection<T>, artifacts: TargetArtifactCatalog) -> Self {
        Self {
            connection: Arc::new(Mutex::new(connection)),
            artifacts,
            targets: BTreeMap::new(),
            capture_ingress: None,
            capture_supervisors: BTreeMap::new(),
            next_capture_generation: 1,
        }
    }

    #[must_use]
    pub fn with_capture_ingress(mut self, ingress: CaptureIngress) -> Self {
        self.capture_ingress = Some(ingress);
        self
    }

    pub fn monitor(&self) -> TargetProcessMonitor<T> {
        TargetProcessMonitor { connection: Arc::clone(&self.connection) }
    }

    pub fn register_target(
        &mut self,
        target_instance_id: TargetInstanceId,
        controller_target_id: OpaqueTargetId,
    ) {
        self.targets
            .insert(target_instance_id, controller_target_id);
    }
}

enum CaptureSupervisorCommand {
    SetPaused {
        paused: bool,
        reply: SyncSender<Result<(), ()>>,
    },
    Finish,
}

struct CaptureSupervisor {
    commands: SyncSender<CaptureSupervisorCommand>,
    worker: Option<JoinHandle<()>>,
}

impl CaptureSupervisor {
    fn start<T: ControllerTransport + Send + 'static>(
        connection: Arc<Mutex<ControllerConnection<T>>>,
        target_id: OpaqueTargetId,
        producer: CaptureProducerConfiguration,
        ingress: CaptureIngress,
    ) -> Result<Self, HostFailure> {
        let (commands, receiver) = sync_channel(8);
        let worker = thread::Builder::new()
            .name("glyphshift-capture-supervisor".into())
            .spawn(move || {
                capture_supervisor_loop(connection, target_id, producer, ingress, receiver);
            })
            .map_err(|_| HostFailure::Unavailable)?;
        Ok(Self {
            commands,
            worker: Some(worker),
        })
    }

    fn set_paused(&self, paused: bool) -> Result<(), HostFailure> {
        let (reply, response) = sync_channel(1);
        self.commands
            .send(CaptureSupervisorCommand::SetPaused { paused, reply })
            .map_err(|_| HostFailure::Unavailable)?;
        response
            .recv()
            .map_err(|_| HostFailure::Unavailable)?
            .map_err(|()| HostFailure::Unavailable)
    }

    fn finish(&mut self) {
        let _ = self.commands.send(CaptureSupervisorCommand::Finish);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Drop for CaptureSupervisor {
    fn drop(&mut self) {
        self.finish();
    }
}

fn capture_supervisor_loop<T: ControllerTransport + Send + 'static>(
    connection: Arc<Mutex<ControllerConnection<T>>>,
    target_id: OpaqueTargetId,
    producer: CaptureProducerConfiguration,
    ingress: CaptureIngress,
    commands: Receiver<CaptureSupervisorCommand>,
) {
    let mut cursor = CaptureObservationCursor::new(&producer);
    loop {
        match commands.recv_timeout(CAPTURE_POLL_INTERVAL) {
            Ok(CaptureSupervisorCommand::SetPaused { paused, reply }) => {
                let result = if paused {
                    drain_observations(&connection, target_id, &mut cursor, &ingress)
                } else {
                    Ok(())
                };
                let _ = reply.send(result);
            }
            Ok(CaptureSupervisorCommand::Finish) => {
                let _ = drain_observations(&connection, target_id, &mut cursor, &ingress);
                return;
            }
            Err(RecvTimeoutError::Timeout) => {
                // A target can transiently reject an observation query while its UI thread is
                // busy. Keep the supervisor alive so the next poll can recover; explicit pause
                // still reports a drain failure to the caller through SetPaused above.
                let _ = drain_observations(&connection, target_id, &mut cursor, &ingress);
            }
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}

fn drain_observations<T: ControllerTransport + Send + 'static>(
    connection: &Arc<Mutex<ControllerConnection<T>>>,
    target_id: OpaqueTargetId,
    cursor: &mut CaptureObservationCursor,
    ingress: &CaptureIngress,
) -> Result<(), ()> {
    for _ in 0..MAX_BATCHES_PER_DRAIN {
        let batch = connection
            .lock()
            .map_err(|_| ())?
            .query_observations(target_id)
            .map_err(|_| ())?;
        let empty = batch.records().is_empty();
        let dropped = cursor.accept(&batch).map_err(|_| ())?;
        ingress.report_dropped(dropped);
        for record in batch.records() {
            let _ = ingress.try_observe(record.adapter_id(), record.source());
        }
        if empty {
            return Ok(());
        }
    }
    Ok(())
}

impl<T: ControllerTransport + Send> TargetProcessHost<T> {
    fn target_id(&self, target: &TargetInstance) -> Result<OpaqueTargetId, HostFailure> {
        self.targets
            .get(target.id())
            .copied()
            .ok_or(HostFailure::Unavailable)
    }

    fn target_deployments(
        &self,
        bindings: &[AdapterBinding],
    ) -> Result<Vec<NativeAdapterDeployment>, HostFailure> {
        bindings
            .iter()
            .filter_map(|binding| {
                let AdapterHostBinding::TargetProcess { library } = &binding.host else {
                    return None;
                };
                Some(
                    self.artifacts
                        .adapters
                        .get(library)
                        .cloned()
                        .ok_or(HostFailure::Unavailable)
                        .and_then(|path| {
                            NativeAdapterDeployment::new(path, binding.clone())
                                .map_err(|_| HostFailure::HandshakeRejected)
                        }),
                )
            })
            .collect()
    }

    fn target_features(bindings: &[AdapterBinding]) -> Vec<BoundFeature> {
        bindings
            .iter()
            .filter(|binding| matches!(binding.host, AdapterHostBinding::TargetProcess { .. }))
            .flat_map(|binding| {
                binding.features.iter().copied().map(|feature| {
                    BoundFeature::new(binding.adapter_id.clone(), binding.version, feature)
                })
            })
            .collect()
    }

    fn target_adapters(bindings: &[AdapterBinding]) -> Vec<BoundAdapter> {
        bindings
            .iter()
            .filter(|binding| matches!(binding.host, AdapterHostBinding::TargetProcess { .. }))
            .map(|binding| BoundAdapter::new(binding.adapter_id.clone(), binding.version))
            .collect()
    }
}

fn host_protocol_failure(error: ControllerProtocolError) -> HostFailure {
    let ControllerProtocolError::Transport(TransportFailure::Rejected(reason)) = error else {
        return HostFailure::Unavailable;
    };
    let reason = match reason {
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
        ControllerRejection::TargetRuntimeRejected(STATUS_TARGET_RUNTIME_INVALID_DEPLOYMENT) => {
            HostOperationFailure::TargetRuntimeRestartRequired
        }
        ControllerRejection::TargetRuntimeRejected(status) => {
            HostOperationFailure::TargetRuntimeRejected(status)
        }
        ControllerRejection::Unknown => HostOperationFailure::ControllerRejected,
    };
    HostFailure::OperationRejected(reason)
}

impl<T: ControllerTransport + Send + 'static> AdapterHostPort for TargetProcessHost<T> {
    fn activate(
        &mut self,
        _target: &TargetInstance,
        _bindings: &[AdapterBinding],
    ) -> Result<HostActivation, HostFailure> {
        Err(HostFailure::HandshakeRejected)
    }

    fn activate_runtime(
        &mut self,
        target: &TargetInstance,
        bindings: &[AdapterBinding],
        publication: &RuntimePublication,
    ) -> Result<HostActivation, HostFailure> {
        let target_id = self.target_id(target)?;
        let mut deployment =
            TargetRuntimeDeployment::new(publication.clone(), self.target_deployments(bindings)?);
        let pending_producer = if self.capture_ingress.is_some() {
            if self.capture_supervisors.contains_key(&target_id) {
                return Err(HostFailure::Unavailable);
            }
            let generation = self.next_capture_generation;
            self.next_capture_generation =
                generation.checked_add(1).ok_or(HostFailure::Unavailable)?;
            let producer = CaptureProducerConfiguration::new(
                CaptureProducerId::new(format!("target-{}", target_id.as_u64()))
                    .map_err(|_| HostFailure::Unavailable)?,
                generation,
            )
            .map_err(|_| HostFailure::Unavailable)?;
            deployment = deployment.with_observation_producer(producer.clone());
            Some(producer)
        } else {
            None
        };
        let deployment_json = deployment
            .encode_json()
            .map_err(|_| HostFailure::HandshakeRejected)?;
        let runtime = self
            .artifacts
            .runtime_for(target.facts().architecture())
            .ok_or(HostFailure::Unavailable)?;
        let runtime_library = runtime.library.to_str().ok_or(HostFailure::Unavailable)?;
        let command = ControllerRuntimeDeployment::new(
            runtime_library,
            runtime.sha256,
            deployment_json,
            publication.generation().value(),
        );
        let ack = self
            .connection
            .lock()
            .map_err(|_| HostFailure::Unavailable)?
            .activate_runtime(target_id, &command)
            .map_err(host_protocol_failure)?;
        let publication_identity = publication
            .identity()
            .map_err(|_| HostFailure::HandshakeRejected)?;
        if ack.generation() != publication.generation().value()
            || ack.publication_identity() != publication_identity.as_bytes()
        {
            return Err(HostFailure::HandshakeRejected);
        }
        if let Some(producer) = pending_producer {
            let ingress = self
                .capture_ingress
                .as_ref()
                .cloned()
                .ok_or(HostFailure::Unavailable)?;
            match CaptureSupervisor::start(self.connection.clone(), target_id, producer, ingress) {
                Ok(supervisor) => {
                    self.capture_supervisors.insert(target_id, supervisor);
                }
                Err(error) => {
                    let _ = self
                        .connection
                        .lock()
                        .map_err(|_| HostFailure::Unavailable)?
                        .deactivate_runtime(target_id);
                    return Err(error);
                }
            }
        }
        if let Some(active_adapter_ids) = ack.active_adapter_ids() {
            let (acknowledged, failed): (Vec<_>, Vec<_>) = Self::target_features(bindings)
                .into_iter()
                .partition(|feature| active_adapter_ids.contains(feature.adapter_id()));
            Ok(HostActivation::reported(acknowledged, failed))
        } else {
            Ok(HostActivation::connected(Self::target_features(bindings)))
        }
    }

    fn update_runtime(
        &mut self,
        _session_id: SessionId,
        target: &TargetInstance,
        _bindings: &[AdapterBinding],
        publication: &RuntimePublication,
    ) -> Result<HostGenerationReport, HostFailure> {
        let target_id = self.target_id(target)?;
        let publication_json = publication
            .encode_json()
            .map_err(|_| HostFailure::HandshakeRejected)?;
        let ack = self
            .connection
            .lock()
            .map_err(|_| HostFailure::Unavailable)?
            .update_runtime(
                target_id,
                &publication_json,
                publication.generation().value(),
            )
            .map_err(host_protocol_failure)?;
        let publication_identity = publication
            .identity()
            .map_err(|_| HostFailure::HandshakeRejected)?;
        if ack.generation() != publication.generation().value()
            || ack.publication_identity() != publication_identity.as_bytes()
        {
            return Err(HostFailure::HandshakeRejected);
        }
        Ok(HostGenerationReport::target_runtime(
            publication.generation(),
        ))
    }

    fn control_capture(
        &mut self,
        _session_id: SessionId,
        target: &TargetInstance,
        paused: bool,
    ) -> Result<(), HostFailure> {
        let target_id = self.target_id(target)?;
        if paused {
            self.connection
                .lock()
                .map_err(|_| HostFailure::Unavailable)?
                .control_capture(target_id, true)
                .map_err(host_protocol_failure)?;
            self.capture_supervisors
                .get(&target_id)
                .ok_or(HostFailure::Unavailable)?
                .set_paused(true)?;
            Ok(())
        } else {
            self.capture_supervisors
                .get(&target_id)
                .ok_or(HostFailure::Unavailable)?
                .set_paused(false)?;
            self.connection
                .lock()
                .map_err(|_| HostFailure::Unavailable)?
                .control_capture(target_id, false)
                .map_err(host_protocol_failure)
        }
    }

    fn control_runtime_diagnostics(
        &mut self,
        _session_id: SessionId,
        target: &TargetInstance,
        enabled: bool,
    ) -> Result<(), HostFailure> {
        let target_id = self.target_id(target)?;
        self.connection
            .lock()
            .map_err(|_| HostFailure::Unavailable)?
            .control_runtime_diagnostics(target_id, enabled)
            .map_err(host_protocol_failure)
    }

    fn query_runtime_diagnostics(
        &mut self,
        _session_id: SessionId,
        target: &TargetInstance,
    ) -> Result<RuntimeTraceBatch, HostFailure> {
        let target_id = self.target_id(target)?;
        let batch = self
            .connection
            .lock()
            .map_err(|_| HostFailure::Unavailable)?
            .query_runtime_diagnostics(target_id)
            .map_err(host_protocol_failure)?;
        Ok(RuntimeTraceBatch::new(
            batch.records().iter().map(|record| {
                RuntimeTraceRecord::new(
                    record.adapter_id(),
                    record.source_text(),
                    match record.status() {
                        ControllerRuntimeTraceStatus::NoMatch => RuntimeTraceStatus::NoMatch,
                        ControllerRuntimeTraceStatus::Matched => RuntimeTraceStatus::Matched,
                        ControllerRuntimeTraceStatus::ContextRecorded => {
                            RuntimeTraceStatus::ContextRecorded
                        }
                        ControllerRuntimeTraceStatus::InvalidObservation => {
                            RuntimeTraceStatus::InvalidObservation
                        }
                        ControllerRuntimeTraceStatus::InvalidRouteProgram => {
                            RuntimeTraceStatus::InvalidRouteProgram
                        }
                        ControllerRuntimeTraceStatus::ExecutionLimitExceeded => {
                            RuntimeTraceStatus::ExecutionLimitExceeded
                        }
                        ControllerRuntimeTraceStatus::StateLimitExceeded => {
                            RuntimeTraceStatus::StateLimitExceeded
                        }
                    },
                    match record.text() {
                        ControllerRuntimeTextOutcome::Unmatched => RuntimeTextOutcome::Unmatched,
                        ControllerRuntimeTextOutcome::Replaced => RuntimeTextOutcome::Replaced,
                    },
                    match record.font() {
                        ControllerRuntimeFontOutcome::Unmatched => RuntimeFontOutcome::Unmatched,
                        ControllerRuntimeFontOutcome::Protected => RuntimeFontOutcome::Protected,
                        ControllerRuntimeFontOutcome::Substituted => {
                            RuntimeFontOutcome::Substituted
                        }
                    },
                    record.generation(),
                    record.publication_identity(),
                    record.translation_digest(),
                    record.font_policy_digest(),
                )
            }),
            batch.dropped(),
        ))
    }

    fn health(
        &mut self,
        _session_id: SessionId,
        _target: &TargetInstance,
        bindings: &[AdapterBinding],
    ) -> Result<HostHealthReport, HostFailure> {
        match self
            .connection
            .lock()
            .map_err(|_| HostFailure::Unavailable)?
            .health()
        {
            ControllerHealth::Available => Ok(HostHealthReport::healthy()),
            ControllerHealth::Degraded => {
                Ok(HostHealthReport::failed(Self::target_adapters(bindings)))
            }
        }
    }

    fn deactivate(
        &mut self,
        _session_id: SessionId,
        target: &TargetInstance,
        bindings: &[AdapterBinding],
        _plan: &[AdapterDeactivation],
    ) -> Result<HostDeactivation, HostFailure> {
        let target_id = self.target_id(target)?;
        if self.capture_ingress.is_some() {
            let _ = self
                .connection
                .lock()
                .map_err(|_| HostFailure::Unavailable)?
                .control_capture(target_id, true);
            if let Some(mut supervisor) = self.capture_supervisors.remove(&target_id) {
                supervisor.finish();
            }
        }
        let deactivation = self
            .connection
            .lock()
            .map_err(|_| HostFailure::Unavailable)?
            .deactivate_runtime(target_id);
        match deactivation {
            Ok(())
            | Err(ControllerProtocolError::Transport(TransportFailure::Rejected(
                ControllerRejection::TargetProcessUnavailable,
            ))) => {}
            Err(error) => return Err(host_protocol_failure(error)),
        }
        Ok(HostDeactivation::completed(Self::target_adapters(bindings)))
    }

    fn release(
        &mut self,
        _session_id: SessionId,
        target: &TargetInstance,
        _bindings: &[AdapterBinding],
    ) -> Result<(), HostFailure> {
        let target_id = self.target_id(target)?;
        if self.capture_ingress.is_some() {
            if let Some(mut supervisor) = self.capture_supervisors.remove(&target_id) {
                supervisor.finish();
            }
        }
        self.targets.remove(target.id());
        Ok(())
    }
}

//! Production Adapter Host for injected target-process Runtime instances.

use glyphshift_adapter_registry::{AdapterBinding, AdapterHostBinding, PackageArtifactId};
use glyphshift_capture::CaptureConfiguration;
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
use glyphshift_target_runtime_contract::{NativeAdapterDeployment, TargetRuntimeDeployment};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

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
            adapters: indexed,
        })
    }
}

fn available_library(path: &Path) -> bool {
    path.is_absolute() && path.is_file()
}

pub struct TargetProcessHost<T> {
    connection: ControllerConnection<T>,
    artifacts: TargetArtifactCatalog,
    targets: BTreeMap<TargetInstanceId, OpaqueTargetId>,
    capture: Option<CaptureConfiguration>,
}

impl<T> TargetProcessHost<T> {
    #[must_use]
    pub fn new(connection: ControllerConnection<T>, artifacts: TargetArtifactCatalog) -> Self {
        Self {
            connection,
            artifacts,
            targets: BTreeMap::new(),
            capture: None,
        }
    }

    #[must_use]
    pub fn with_capture(mut self, capture: CaptureConfiguration) -> Self {
        self.capture = Some(capture);
        self
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
        ControllerRejection::TargetRuntimeRejected(status) => {
            HostOperationFailure::TargetRuntimeRejected(status)
        }
        ControllerRejection::Unknown => HostOperationFailure::ControllerRejected,
    };
    HostFailure::OperationRejected(reason)
}

impl<T: ControllerTransport + Send> AdapterHostPort for TargetProcessHost<T> {
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
        if let Some(capture) = self.capture.clone() {
            deployment = deployment.with_capture(capture);
        }
        let deployment_json = deployment
            .encode_json()
            .map_err(|_| HostFailure::HandshakeRejected)?;
        let runtime_library = self
            .artifacts
            .runtime
            .library
            .to_str()
            .ok_or(HostFailure::Unavailable)?;
        let command = ControllerRuntimeDeployment::new(
            runtime_library,
            self.artifacts.runtime.sha256,
            deployment_json,
            publication.generation().value(),
        );
        let ack = self
            .connection
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
        Ok(HostActivation::connected(Self::target_features(bindings)))
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
        self.connection
            .control_capture(target_id, paused)
            .map_err(host_protocol_failure)
    }

    fn control_runtime_diagnostics(
        &mut self,
        _session_id: SessionId,
        target: &TargetInstance,
        enabled: bool,
    ) -> Result<(), HostFailure> {
        let target_id = self.target_id(target)?;
        self.connection
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
        match self.connection.health() {
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
        self.connection
            .deactivate_runtime(target_id)
            .map_err(host_protocol_failure)?;
        Ok(HostDeactivation::completed(Self::target_adapters(bindings)))
    }

    fn release(
        &mut self,
        _session_id: SessionId,
        target: &TargetInstance,
        _bindings: &[AdapterBinding],
    ) -> Result<(), HostFailure> {
        self.targets.remove(target.id());
        Ok(())
    }
}

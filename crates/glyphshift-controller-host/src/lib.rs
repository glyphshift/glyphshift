//! Verified isolated-process host implementing the Controller transport contract.

use glyphshift_adapter_registry::{AdapterRequirement, AdapterVersion, AdapterVersionRequirement};
use glyphshift_controller_sdk::{
    Request, RequestEnvelope, Response, ResponseEnvelope, WireAdapterRequirement,
    WireControllerConfiguration, WireControllerLossPolicy, WireFeature, WireOperation,
    WireRuntimeDeployment, WireRuntimeFontOutcome, WireRuntimeTextOutcome, WireRuntimeTraceStatus,
    PROTOCOL_SCHEMA,
};
use glyphshift_domain::{AdapterId, Feature, TargetFacts};
use glyphshift_extension::{CodeHash, ControllerCodeIdentity, ExtensionId, ProtocolVersion};
use glyphshift_protocol::{
    ControllerHello, ControllerInstallation, ControllerInstallationToken, ControllerInventory,
    ControllerLaunchAck, ControllerNonce, ControllerOperation, ControllerRecipe,
    ControllerRuntimeAck, ControllerRuntimeDeployment, ControllerRuntimeFontOutcome,
    ControllerRuntimeTextOutcome, ControllerRuntimeTraceBatch, ControllerRuntimeTraceRecord,
    ControllerRuntimeTraceStatus, ControllerTarget, ControllerTargetToken, ControllerTransport,
    RecipeControllerLossPolicy, RecipeDirective, TransportFailure,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread::{self, JoinHandle};
use std::time::Duration;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControllerTrustPolicy {
    trusted_signers: BTreeSet<Box<str>>,
}

impl ControllerTrustPolicy {
    #[must_use]
    pub fn new(trusted_signers: impl IntoIterator<Item = impl Into<Box<str>>>) -> Self {
        Self {
            trusted_signers: trusted_signers.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControllerLoadError {
    ArtifactUnavailable,
    UntrustedSigner,
    HashMismatch,
    SpawnFailed,
}

#[derive(Clone, Debug)]
pub struct VerifiedControllerArtifact {
    executable: PathBuf,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ControllerStartupConfig {
    executable_names: Vec<Box<str>>,
    executable_paths: Vec<Box<str>>,
    descendant_executable_names: Vec<Box<str>>,
    adapter_requirements: Vec<AdapterRequirement>,
}

impl ControllerStartupConfig {
    #[must_use]
    pub fn new(
        executable_names: impl IntoIterator<Item = impl Into<Box<str>>>,
        adapter_requirements: impl IntoIterator<Item = AdapterRequirement>,
    ) -> Self {
        Self {
            executable_names: executable_names.into_iter().map(Into::into).collect(),
            executable_paths: Vec::new(),
            descendant_executable_names: Vec::new(),
            adapter_requirements: adapter_requirements.into_iter().collect(),
        }
    }

    #[must_use]
    pub fn with_executable_paths(
        mut self,
        executable_paths: impl IntoIterator<Item = impl Into<Box<str>>>,
    ) -> Self {
        self.executable_paths = executable_paths.into_iter().map(Into::into).collect();
        self
    }

    #[must_use]
    pub fn with_descendant_executable_names(
        mut self,
        executable_names: impl IntoIterator<Item = impl Into<Box<str>>>,
    ) -> Self {
        self.descendant_executable_names = executable_names.into_iter().map(Into::into).collect();
        self
    }

    fn to_wire(&self) -> WireControllerConfiguration {
        WireControllerConfiguration {
            executable_names: self
                .executable_names
                .iter()
                .map(|name| name.to_string())
                .collect(),
            executable_paths: self
                .executable_paths
                .iter()
                .map(|path| path.to_string())
                .collect(),
            descendant_executable_names: self
                .descendant_executable_names
                .iter()
                .map(|name| name.to_string())
                .collect(),
            adapter_requirements: self
                .adapter_requirements
                .iter()
                .map(requirement_to_wire)
                .collect(),
        }
    }
}

impl VerifiedControllerArtifact {
    pub fn verify(
        executable: impl Into<PathBuf>,
        identity: &ControllerCodeIdentity,
        trust: &ControllerTrustPolicy,
    ) -> Result<Self, ControllerLoadError> {
        if !trust
            .trusted_signers
            .contains(identity.signer_id().as_str())
        {
            return Err(ControllerLoadError::UntrustedSigner);
        }
        let executable = executable.into();
        let observed = measure_code_hash(&executable)?;
        if observed != identity.hash() {
            return Err(ControllerLoadError::HashMismatch);
        }
        Ok(Self { executable })
    }
}

pub fn measure_code_hash(path: &Path) -> Result<CodeHash, ControllerLoadError> {
    let mut file = File::open(path).map_err(|_| ControllerLoadError::ArtifactUnavailable)?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|_| ControllerLoadError::ArtifactUnavailable)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(CodeHash::new(digest.finalize().into()))
}

pub struct ProcessControllerTransport {
    child: Child,
    input: Option<ChildStdin>,
    responses: Receiver<String>,
    reader: Option<JoinHandle<()>>,
    timeout: Duration,
    next_request_id: u64,
    terminated: bool,
    configuration: ControllerStartupConfig,
}

impl ProcessControllerTransport {
    pub fn spawn(
        artifact: VerifiedControllerArtifact,
        timeout: Duration,
    ) -> Result<Self, ControllerLoadError> {
        Self::spawn_configured(artifact, timeout, ControllerStartupConfig::default())
    }

    pub fn spawn_configured(
        artifact: VerifiedControllerArtifact,
        timeout: Duration,
        configuration: ControllerStartupConfig,
    ) -> Result<Self, ControllerLoadError> {
        let mut command = Command::new(artifact.executable);
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        hide_window(&mut command);
        let mut child = command
            .spawn()
            .map_err(|_| ControllerLoadError::SpawnFailed)?;
        let input = child.stdin.take().ok_or(ControllerLoadError::SpawnFailed)?;
        let output = child
            .stdout
            .take()
            .ok_or(ControllerLoadError::SpawnFailed)?;
        let (sender, responses) = mpsc::channel();
        let reader = thread::spawn(move || {
            for line in BufReader::new(output).lines() {
                let Ok(line) = line else {
                    break;
                };
                if sender.send(line).is_err() {
                    break;
                }
            }
        });
        Ok(Self {
            child,
            input: Some(input),
            responses,
            reader: Some(reader),
            timeout,
            next_request_id: 0,
            terminated: false,
            configuration,
        })
    }

    fn round_trip(&mut self, request: Request) -> Result<Response, TransportFailure> {
        if self.terminated {
            return Err(TransportFailure::Crashed);
        }
        self.next_request_id = self.next_request_id.saturating_add(1);
        let request_id = self.next_request_id;
        let envelope = RequestEnvelope {
            schema: PROTOCOL_SCHEMA.into(),
            request_id,
            request,
        };
        let Some(input) = self.input.as_mut() else {
            return Err(TransportFailure::Crashed);
        };
        serde_json::to_writer(&mut *input, &envelope)
            .map_err(|_| TransportFailure::MalformedMessage)?;
        input
            .write_all(b"\n")
            .and_then(|()| input.flush())
            .map_err(|_| TransportFailure::Crashed)?;
        let line = self.responses.recv_timeout(self.timeout).map_err(|error| {
            if error == mpsc::RecvTimeoutError::Timeout {
                TransportFailure::Timeout
            } else {
                TransportFailure::Crashed
            }
        })?;
        let response: ResponseEnvelope =
            serde_json::from_str(&line).map_err(|_| TransportFailure::MalformedMessage)?;
        if response.schema != PROTOCOL_SCHEMA || response.request_id != request_id {
            return Err(TransportFailure::MalformedMessage);
        }
        if let Response::Error { code } = &response.response {
            return Err(TransportFailure::Rejected(classify_rejection(code)));
        }
        Ok(response.response)
    }
}

fn classify_rejection(code: &str) -> glyphshift_protocol::ControllerRejection {
    use glyphshift_protocol::ControllerRejection;

    let reason = code.split_once(':').map_or(code, |(_, reason)| reason);
    match reason {
        "process_unavailable" => ControllerRejection::TargetProcessUnavailable,
        "remote_allocation_failed" | "remote_write_failed" | "remote_read_failed" => {
            ControllerRejection::RemoteMemoryUnavailable
        }
        "runtime_module_unavailable" => ControllerRejection::RuntimeModuleUnavailable,
        "runtime_export_unavailable" => ControllerRejection::RuntimeExportUnavailable,
        "remote_thread_failed" => ControllerRejection::RemoteThreadUnavailable,
        "remote_thread_timeout" => ControllerRejection::RemoteThreadTimeout,
        _ => reason
            .strip_prefix("target_runtime_rejected_")
            .and_then(|status| status.parse().ok())
            .map_or(
                ControllerRejection::Unknown,
                ControllerRejection::TargetRuntimeRejected,
            ),
    }
}

impl ControllerTransport for ProcessControllerTransport {
    fn handshake(
        &mut self,
        expected_extension: &ExtensionId,
        version: ProtocolVersion,
        nonce: ControllerNonce,
    ) -> Result<ControllerHello, TransportFailure> {
        match self.round_trip(Request::Handshake {
            extension_id: expected_extension.as_str().into(),
            version_major: version.major(),
            version_minor: version.minor(),
            nonce: nonce.as_bytes(),
            configuration: self.configuration.to_wire(),
        })? {
            Response::Hello {
                extension_id,
                version_major,
                version_minor,
                nonce,
            } => Ok(ControllerHello::new(
                ExtensionId::new(extension_id),
                ProtocolVersion::new(version_major, version_minor),
                ControllerNonce::new(nonce),
            )),
            _ => Err(TransportFailure::MalformedMessage),
        }
    }

    fn inventory(&mut self) -> Result<ControllerInventory, TransportFailure> {
        let Response::Inventory { inventory } = self.round_trip(Request::Inventory)? else {
            return Err(TransportFailure::MalformedMessage);
        };
        Ok(ControllerInventory::new(
            inventory.installations.into_iter().map(|installation| {
                ControllerInstallation::new(
                    ControllerInstallationToken::new(installation.token),
                    installation.display_name,
                )
            }),
            inventory.targets.into_iter().map(|target| {
                ControllerTarget::new(
                    ControllerTargetToken::new(target.token),
                    target.display_name,
                    TargetFacts::new(target.operating_system, target.architecture),
                )
            }),
        ))
    }

    fn launch(
        &mut self,
        installation: &ControllerInstallationToken,
    ) -> Result<ControllerLaunchAck, TransportFailure> {
        match self.round_trip(Request::Launch {
            installation_token: installation.as_str().into(),
        })? {
            Response::LaunchAccepted => Ok(ControllerLaunchAck::accepted()),
            _ => Err(TransportFailure::MalformedMessage),
        }
    }

    fn prepare(
        &mut self,
        target: &ControllerTargetToken,
        requested_features: &BTreeSet<Feature>,
    ) -> Result<ControllerRecipe, TransportFailure> {
        let Response::Recipe { recipe } = self.round_trip(Request::Prepare {
            target_token: target.as_str().into(),
            requested_features: requested_features
                .iter()
                .copied()
                .map(feature_to_wire)
                .collect(),
        })?
        else {
            return Err(TransportFailure::MalformedMessage);
        };
        let directives = recipe
            .adapters
            .into_iter()
            .map(|adapter| {
                RecipeDirective::adapter(AdapterRequirement::new(
                    AdapterId::new(adapter.adapter_id),
                    AdapterVersionRequirement::Exact(AdapterVersion::new(
                        adapter.version_major,
                        adapter.version_minor,
                        adapter.version_patch,
                    )),
                    adapter.features.into_iter().map(feature_from_wire),
                ))
            })
            .collect::<Vec<_>>();
        let loss_policy = match recipe.controller_loss_policy {
            WireControllerLossPolicy::Continue => RecipeControllerLossPolicy::Continue,
            WireControllerLossPolicy::Degrade => RecipeControllerLossPolicy::Degrade,
        };
        Ok(ControllerRecipe::new(directives, loss_policy))
    }

    fn activate_runtime(
        &mut self,
        target: &ControllerTargetToken,
        deployment: &ControllerRuntimeDeployment,
    ) -> Result<ControllerRuntimeAck, TransportFailure> {
        match self.round_trip(Request::ActivateRuntime {
            target_token: target.as_str().into(),
            deployment: WireRuntimeDeployment {
                runtime_library: deployment.runtime_library().into(),
                runtime_library_sha256: deployment.runtime_library_sha256(),
                deployment_json: deployment.deployment_json().into(),
                generation: deployment.generation(),
            },
        })? {
            Response::RuntimeActivated {
                generation,
                publication_identity,
            } => Ok(ControllerRuntimeAck::new(generation, publication_identity)),
            _ => Err(TransportFailure::MalformedMessage),
        }
    }

    fn update_runtime(
        &mut self,
        target: &ControllerTargetToken,
        publication_json: &str,
        generation: u64,
    ) -> Result<ControllerRuntimeAck, TransportFailure> {
        match self.round_trip(Request::UpdateRuntime {
            target_token: target.as_str().into(),
            publication_json: publication_json.into(),
            generation,
        })? {
            Response::RuntimeUpdated {
                generation,
                publication_identity,
            } => Ok(ControllerRuntimeAck::new(generation, publication_identity)),
            _ => Err(TransportFailure::MalformedMessage),
        }
    }

    fn control_capture(
        &mut self,
        target: &ControllerTargetToken,
        paused: bool,
    ) -> Result<(), TransportFailure> {
        match self.round_trip(Request::ControlCapture {
            target_token: target.as_str().into(),
            paused,
        })? {
            Response::CaptureControlled {
                paused: acknowledged,
            } if acknowledged == paused => Ok(()),
            _ => Err(TransportFailure::MalformedMessage),
        }
    }

    fn control_runtime_diagnostics(
        &mut self,
        target: &ControllerTargetToken,
        enabled: bool,
    ) -> Result<(), TransportFailure> {
        match self.round_trip(Request::ControlDiagnostics {
            target_token: target.as_str().into(),
            enabled,
        })? {
            Response::DiagnosticsControlled {
                enabled: acknowledged,
            } if acknowledged == enabled => Ok(()),
            _ => Err(TransportFailure::MalformedMessage),
        }
    }

    fn query_runtime_diagnostics(
        &mut self,
        target: &ControllerTargetToken,
    ) -> Result<ControllerRuntimeTraceBatch, TransportFailure> {
        let Response::RuntimeDiagnostics { batch } =
            self.round_trip(Request::QueryDiagnostics {
                target_token: target.as_str().into(),
            })?
        else {
            return Err(TransportFailure::MalformedMessage);
        };
        Ok(ControllerRuntimeTraceBatch::new(
            batch.records.into_iter().map(|record| {
                ControllerRuntimeTraceRecord::new(
                    record.adapter_id,
                    record.source_text,
                    match record.status {
                        WireRuntimeTraceStatus::NoMatch => ControllerRuntimeTraceStatus::NoMatch,
                        WireRuntimeTraceStatus::Matched => ControllerRuntimeTraceStatus::Matched,
                        WireRuntimeTraceStatus::ContextRecorded => {
                            ControllerRuntimeTraceStatus::ContextRecorded
                        }
                        WireRuntimeTraceStatus::InvalidObservation => {
                            ControllerRuntimeTraceStatus::InvalidObservation
                        }
                        WireRuntimeTraceStatus::InvalidRouteProgram => {
                            ControllerRuntimeTraceStatus::InvalidRouteProgram
                        }
                        WireRuntimeTraceStatus::ExecutionLimitExceeded => {
                            ControllerRuntimeTraceStatus::ExecutionLimitExceeded
                        }
                        WireRuntimeTraceStatus::StateLimitExceeded => {
                            ControllerRuntimeTraceStatus::StateLimitExceeded
                        }
                    },
                    match record.text {
                        WireRuntimeTextOutcome::Unmatched => {
                            ControllerRuntimeTextOutcome::Unmatched
                        }
                        WireRuntimeTextOutcome::Replaced => ControllerRuntimeTextOutcome::Replaced,
                    },
                    match record.font {
                        WireRuntimeFontOutcome::Unmatched => {
                            ControllerRuntimeFontOutcome::Unmatched
                        }
                        WireRuntimeFontOutcome::Protected => {
                            ControllerRuntimeFontOutcome::Protected
                        }
                        WireRuntimeFontOutcome::Substituted => {
                            ControllerRuntimeFontOutcome::Substituted
                        }
                    },
                    record.generation,
                    record.publication_identity,
                    record.translation_digest,
                    record.font_policy_digest,
                )
            }),
            batch.dropped,
        ))
    }

    fn deactivate_runtime(
        &mut self,
        target: &ControllerTargetToken,
    ) -> Result<(), TransportFailure> {
        match self.round_trip(Request::DeactivateRuntime {
            target_token: target.as_str().into(),
        })? {
            Response::RuntimeDeactivated => Ok(()),
            _ => Err(TransportFailure::MalformedMessage),
        }
    }

    fn cancel(&mut self, operation: &ControllerOperation) -> Result<(), TransportFailure> {
        let operation = match operation {
            ControllerOperation::Inventory => WireOperation::Inventory,
            ControllerOperation::Prepare(target) => WireOperation::Prepare {
                target_token: target.as_str().into(),
            },
        };
        match self.round_trip(Request::Cancel { operation })? {
            Response::Cancelled => Ok(()),
            _ => Err(TransportFailure::MalformedMessage),
        }
    }

    fn terminate(&mut self) {
        if self.terminated {
            return;
        }
        self.terminated = true;
        if let Some(mut input) = self.input.take() {
            self.next_request_id = self.next_request_id.saturating_add(1);
            let _ = serde_json::to_writer(
                &mut input,
                &RequestEnvelope {
                    schema: PROTOCOL_SCHEMA.into(),
                    request_id: self.next_request_id,
                    request: Request::Terminate,
                },
            );
            let _ = input.write_all(b"\n");
            let _ = input.flush();
        }
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}

impl Drop for ProcessControllerTransport {
    fn drop(&mut self) {
        self.terminate();
    }
}

fn feature_to_wire(feature: Feature) -> WireFeature {
    match feature {
        Feature::TextObserve => WireFeature::TextObserve,
        Feature::TextReplace => WireFeature::TextReplace,
        Feature::FontSubstitute => WireFeature::FontSubstitute,
        Feature::LayoutAdjust => WireFeature::LayoutAdjust,
        Feature::ResourceReplace => WireFeature::ResourceReplace,
    }
}

fn requirement_to_wire(requirement: &AdapterRequirement) -> WireAdapterRequirement {
    let AdapterVersionRequirement::Exact(version) = requirement.version_requirement();
    WireAdapterRequirement {
        adapter_id: requirement.adapter_id().as_str().into(),
        version_major: version.major(),
        version_minor: version.minor(),
        version_patch: version.patch(),
        features: requirement.features().map(feature_to_wire).collect(),
    }
}

fn feature_from_wire(feature: WireFeature) -> Feature {
    match feature {
        WireFeature::TextObserve => Feature::TextObserve,
        WireFeature::TextReplace => Feature::TextReplace,
        WireFeature::FontSubstitute => Feature::FontSubstitute,
        WireFeature::LayoutAdjust => Feature::LayoutAdjust,
        WireFeature::ResourceReplace => Feature::ResourceReplace,
    }
}

#[cfg(windows)]
fn hide_window(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn hide_window(_command: &mut Command) {}

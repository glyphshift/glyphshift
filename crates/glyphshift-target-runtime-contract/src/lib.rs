//! Versioned deployment contract consumed by the injected target Runtime Host.

use glyphshift_adapter_registry::{
    AdapterBinding, AdapterHostBinding, ArtifactHash, PackageArtifactId,
};
use glyphshift_adapter_sdk::{AdapterDescriptor, AdapterVersion};
use glyphshift_capture::{
    CaptureConfiguration, CaptureProducerConfiguration, CaptureProducerId, CaptureSessionId,
    MAX_OBSERVATION_BATCH_BYTES,
};
use glyphshift_decision::{DecisionTrace, DecisionTraceStatus, FontTrace, TextTrace};
use glyphshift_domain::{AbiVersion, AdapterId, ApplyModel, Feature, Placement};
use glyphshift_runtime_contract::{RuntimePublication, RuntimeWireError};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const DEPLOYMENT_SCHEMA: &str = "glyphshift.target-runtime/3";
const ACTIVATION_REPORT_SCHEMA: &str = "glyphshift.target-runtime-activation/1";
const TRACE_BATCH_SCHEMA: &str = "glyphshift.runtime-trace/1";
pub const STATUS_TARGET_RUNTIME_OK: u32 = 0;
pub const STATUS_TARGET_RUNTIME_INVALID_COMMAND: u32 = 1;
pub const STATUS_TARGET_RUNTIME_INVALID_DEPLOYMENT: u32 = 2;
pub const STATUS_TARGET_RUNTIME_ACTIVATION_FAILED: u32 = 3;
pub const STATUS_TARGET_RUNTIME_UPDATE_FAILED: u32 = 4;
pub const STATUS_TARGET_RUNTIME_ALREADY_ACTIVE: u32 = 10;
pub const STATUS_TARGET_RUNTIME_ADAPTER_LOAD_FAILED: u32 = 11;
pub const STATUS_TARGET_RUNTIME_ADAPTER_CHANGED: u32 = 12;
pub const STATUS_TARGET_RUNTIME_KERNEL_ACTIVATION_FAILED: u32 = 13;
pub const STATUS_TARGET_RUNTIME_ADAPTER_ACTIVATION_FAILED: u32 = 14;
pub const STATUS_TARGET_RUNTIME_UNAVAILABLE: u32 = 15;
pub const STATUS_TARGET_RUNTIME_UPDATE_REJECTED: u32 = 16;
pub const STATUS_TARGET_RUNTIME_CAPTURE_FAILED: u32 = 17;
pub const STATUS_TARGET_RUNTIME_OUTPUT_TOO_SMALL: u32 = 18;
pub const MAX_RUNTIME_TRACE_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_RUNTIME_OBSERVATION_BYTES: usize = MAX_OBSERVATION_BATCH_BYTES;
pub const MAX_RUNTIME_ACTIVATION_REPORT_BYTES: usize = 256 * 1024;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RuntimeCommandV1 {
    pub struct_size: u32,
    pub json: *const u8,
    pub json_len: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RuntimeDiagnosticsQueryV1 {
    pub struct_size: u32,
    pub output: *mut u8,
    pub output_capacity: u32,
    pub output_len: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RuntimeObservationQueryV1 {
    pub struct_size: u32,
    pub output: *mut u8,
    pub output_capacity: u32,
    pub output_len: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RuntimeActivationQueryV1 {
    pub struct_size: u32,
    pub output: *mut u8,
    pub output_capacity: u32,
    pub output_len: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeActivationReport {
    active_adapter_ids: Vec<Box<str>>,
}

impl RuntimeActivationReport {
    #[must_use]
    pub fn new(active_adapter_ids: impl IntoIterator<Item = impl Into<Box<str>>>) -> Self {
        let mut active_adapter_ids = active_adapter_ids
            .into_iter()
            .map(Into::into)
            .collect::<Vec<_>>();
        active_adapter_ids.sort_unstable();
        active_adapter_ids.dedup();
        Self { active_adapter_ids }
    }

    pub fn active_adapter_ids(&self) -> impl Iterator<Item = &str> {
        self.active_adapter_ids.iter().map(AsRef::as_ref)
    }

    pub fn encode_json(&self) -> Result<String, DeploymentError> {
        serde_json::to_string(&WireRuntimeActivationReport {
            schema: ACTIVATION_REPORT_SCHEMA.into(),
            active_adapter_ids: self.active_adapter_ids.clone(),
        })
        .map_err(|_| DeploymentError::InvalidJson)
    }

    pub fn decode_json(json: &str) -> Result<Self, DeploymentError> {
        let wire: WireRuntimeActivationReport =
            serde_json::from_str(json).map_err(|_| DeploymentError::InvalidJson)?;
        if wire.schema.as_ref() != ACTIVATION_REPORT_SCHEMA {
            return Err(DeploymentError::UnsupportedSchema);
        }
        Ok(Self::new(wire.active_adapter_ids))
    }
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct WireRuntimeActivationReport {
    schema: Box<str>,
    active_adapter_ids: Vec<Box<str>>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RuntimeDiagnosticsControl {
    enabled: bool,
}

impl RuntimeDiagnosticsControl {
    #[must_use]
    pub const fn new(enabled: bool) -> Self {
        Self { enabled }
    }

    #[must_use]
    pub const fn enabled(self) -> bool {
        self.enabled
    }

    pub fn encode_json(self) -> Result<String, DeploymentError> {
        serde_json::to_string(&self).map_err(|_| DeploymentError::InvalidJson)
    }

    pub fn decode_json(json: &str) -> Result<Self, DeploymentError> {
        serde_json::from_str(json).map_err(|_| DeploymentError::InvalidJson)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeTraceStatus {
    NoMatch,
    Matched,
    ContextRecorded,
    InvalidObservation,
    InvalidRouteProgram,
    ExecutionLimitExceeded,
    StateLimitExceeded,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeTextOutcome {
    Unmatched,
    Replaced,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeFontOutcome {
    Unmatched,
    Protected,
    Substituted,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
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
    pub fn from_decision(
        adapter_id: impl Into<Box<str>>,
        source_text: impl Into<Box<str>>,
        trace: DecisionTrace,
        publication_identity: glyphshift_runtime_contract::RuntimePublicationIdentity,
    ) -> Self {
        Self::new(
            adapter_id,
            source_text,
            trace.status().into(),
            trace.text().into(),
            trace.font().into(),
            trace.generation().value(),
            publication_identity.as_bytes(),
            trace.translation_digest().as_bytes(),
            trace.font_policy_digest().as_bytes(),
        )
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

impl From<DecisionTraceStatus> for RuntimeTraceStatus {
    fn from(value: DecisionTraceStatus) -> Self {
        match value {
            DecisionTraceStatus::NoMatch => Self::NoMatch,
            DecisionTraceStatus::Matched => Self::Matched,
            DecisionTraceStatus::ContextRecorded => Self::ContextRecorded,
            DecisionTraceStatus::InvalidObservation => Self::InvalidObservation,
            DecisionTraceStatus::InvalidRouteProgram => Self::InvalidRouteProgram,
            DecisionTraceStatus::ExecutionLimitExceeded => Self::ExecutionLimitExceeded,
            DecisionTraceStatus::StateLimitExceeded => Self::StateLimitExceeded,
        }
    }
}

impl From<TextTrace> for RuntimeTextOutcome {
    fn from(value: TextTrace) -> Self {
        match value {
            TextTrace::Unmatched => Self::Unmatched,
            TextTrace::Replaced => Self::Replaced,
        }
    }
}

impl From<FontTrace> for RuntimeFontOutcome {
    fn from(value: FontTrace) -> Self {
        match value {
            FontTrace::Unmatched => Self::Unmatched,
            FontTrace::Protected => Self::Protected,
            FontTrace::Substituted => Self::Substituted,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
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

    pub fn encode_json(&self) -> Result<String, DeploymentError> {
        serde_json::to_string(&WireRuntimeTraceBatch {
            schema: TRACE_BATCH_SCHEMA.into(),
            records: self.records.clone(),
            dropped: self.dropped,
        })
        .map_err(|_| DeploymentError::InvalidJson)
    }

    pub fn decode_json(json: &str) -> Result<Self, DeploymentError> {
        let wire: WireRuntimeTraceBatch =
            serde_json::from_str(json).map_err(|_| DeploymentError::InvalidJson)?;
        if wire.schema.as_ref() != TRACE_BATCH_SCHEMA {
            return Err(DeploymentError::UnsupportedSchema);
        }
        Ok(Self::new(wire.records, wire.dropped))
    }
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct WireRuntimeTraceBatch {
    schema: Box<str>,
    records: Vec<RuntimeTraceRecord>,
    dropped: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CaptureRuntimeControl {
    paused: bool,
}

impl CaptureRuntimeControl {
    #[must_use]
    pub const fn new(paused: bool) -> Self {
        Self { paused }
    }

    #[must_use]
    pub const fn paused(self) -> bool {
        self.paused
    }

    pub fn decode_json(json: &str) -> Result<Self, DeploymentError> {
        serde_json::from_str(json).map_err(|_| DeploymentError::InvalidJson)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeAdapterDeployment {
    library: PathBuf,
    binding: AdapterBinding,
}

impl NativeAdapterDeployment {
    pub fn new(
        library: impl Into<PathBuf>,
        binding: AdapterBinding,
    ) -> Result<Self, DeploymentError> {
        if !matches!(binding.host, AdapterHostBinding::TargetProcess { .. }) {
            return Err(DeploymentError::UnsupportedBinding);
        }
        Ok(Self {
            library: library.into(),
            binding,
        })
    }

    #[must_use]
    pub fn library(&self) -> &std::path::Path {
        &self.library
    }

    #[must_use]
    pub const fn binding(&self) -> &AdapterBinding {
        &self.binding
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TargetRuntimeDeployment {
    publication: RuntimePublication,
    adapters: Vec<NativeAdapterDeployment>,
    capture: Option<CaptureConfiguration>,
    observation_producer: Option<CaptureProducerConfiguration>,
}

impl TargetRuntimeDeployment {
    #[must_use]
    pub fn new(
        publication: RuntimePublication,
        adapters: impl IntoIterator<Item = NativeAdapterDeployment>,
    ) -> Self {
        Self {
            publication,
            adapters: adapters.into_iter().collect(),
            capture: None,
            observation_producer: None,
        }
    }

    #[must_use]
    pub fn with_capture(mut self, capture: CaptureConfiguration) -> Self {
        self.capture = Some(capture);
        self.observation_producer = None;
        self
    }

    #[must_use]
    pub fn with_observation_producer(mut self, producer: CaptureProducerConfiguration) -> Self {
        self.capture = None;
        self.observation_producer = Some(producer);
        self
    }

    #[must_use]
    pub const fn publication(&self) -> &RuntimePublication {
        &self.publication
    }

    #[must_use]
    pub fn adapters(&self) -> &[NativeAdapterDeployment] {
        &self.adapters
    }

    #[must_use]
    pub const fn capture(&self) -> Option<&CaptureConfiguration> {
        self.capture.as_ref()
    }

    #[must_use]
    pub const fn observation_producer(&self) -> Option<&CaptureProducerConfiguration> {
        self.observation_producer.as_ref()
    }

    pub fn encode_json(&self) -> Result<String, DeploymentError> {
        let publication = self
            .publication
            .encode_json()
            .map_err(DeploymentError::Runtime)?;
        let adapters = self
            .adapters
            .iter()
            .map(WireAdapterDeployment::from_deployment)
            .collect::<Result<Vec<_>, _>>()?;
        serde_json::to_string(&WireDeployment {
            schema: DEPLOYMENT_SCHEMA.into(),
            publication,
            adapters,
            capture: self.capture.as_ref().map(|capture| WireCapture {
                session_id: capture.session_id().as_str().into(),
                output_path: capture.output_path().to_path_buf(),
                max_entries: capture.max_entries(),
            }),
            observation_producer: self.observation_producer.as_ref().map(|producer| {
                WireObservationProducer {
                    producer_id: producer.producer_id().as_str().into(),
                    generation: producer.generation(),
                }
            }),
        })
        .map_err(|_| DeploymentError::InvalidJson)
    }

    pub fn decode_json(json: &str) -> Result<Self, DeploymentError> {
        let wire: WireDeployment =
            serde_json::from_str(json).map_err(|_| DeploymentError::InvalidJson)?;
        if wire.schema.as_ref() != DEPLOYMENT_SCHEMA {
            return Err(DeploymentError::UnsupportedSchema);
        }
        let publication =
            RuntimePublication::decode_json(&wire.publication).map_err(DeploymentError::Runtime)?;
        let adapters = wire
            .adapters
            .into_iter()
            .map(WireAdapterDeployment::into_deployment)
            .collect::<Result<Vec<_>, _>>()?;
        let mut deployment = Self::new(publication, adapters);
        if wire.capture.is_some() && wire.observation_producer.is_some() {
            return Err(DeploymentError::InvalidCapture);
        }
        if let Some(capture) = wire.capture {
            deployment = deployment.with_capture(
                CaptureConfiguration::new(
                    CaptureSessionId::new(capture.session_id)
                        .map_err(|_| DeploymentError::InvalidCapture)?,
                    capture.output_path,
                    capture.max_entries,
                )
                .map_err(|_| DeploymentError::InvalidCapture)?,
            );
        }
        if let Some(producer) = wire.observation_producer {
            deployment = deployment.with_observation_producer(
                CaptureProducerConfiguration::new(
                    CaptureProducerId::new(producer.producer_id)
                        .map_err(|_| DeploymentError::InvalidCapture)?,
                    producer.generation,
                )
                .map_err(|_| DeploymentError::InvalidCapture)?,
            );
        }
        Ok(deployment)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeploymentError {
    InvalidJson,
    UnsupportedSchema,
    UnsupportedBinding,
    InvalidDescriptor,
    InvalidCapture,
    Runtime(RuntimeWireError),
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireDeployment {
    schema: Box<str>,
    publication: String,
    adapters: Vec<WireAdapterDeployment>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    capture: Option<WireCapture>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    observation_producer: Option<WireObservationProducer>,
}

#[derive(Serialize, Deserialize)]
struct WireCapture {
    session_id: Box<str>,
    output_path: PathBuf,
    max_entries: u32,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireObservationProducer {
    producer_id: Box<str>,
    generation: u64,
}

#[derive(Serialize, Deserialize)]
struct WireAdapterDeployment {
    library: PathBuf,
    artifact_sha256: [u8; 32],
    artifact_id: Box<str>,
    adapter_id: Box<str>,
    version: [u16; 3],
    apply_model: WireApplyModel,
    placement: WirePlacement,
    descriptor_features: Vec<WireFeature>,
    requested_features: Vec<WireFeature>,
    platforms: Vec<Box<str>>,
    architectures: Vec<Box<str>>,
    abi: [u16; 2],
}

impl WireAdapterDeployment {
    fn from_deployment(value: &NativeAdapterDeployment) -> Result<Self, DeploymentError> {
        let AdapterHostBinding::TargetProcess { library } = &value.binding.host else {
            return Err(DeploymentError::UnsupportedBinding);
        };
        let descriptor = &value.binding.descriptor;
        Ok(Self {
            library: value.library.clone(),
            artifact_sha256: value.binding.artifact_hash.as_bytes(),
            artifact_id: library.as_str().into(),
            adapter_id: descriptor.adapter_id().as_str().into(),
            version: [
                descriptor.version().major(),
                descriptor.version().minor(),
                descriptor.version().patch(),
            ],
            apply_model: descriptor.apply_model().into(),
            placement: descriptor.placement().into(),
            descriptor_features: descriptor.features().map(Into::into).collect(),
            requested_features: value
                .binding
                .features
                .iter()
                .copied()
                .map(Into::into)
                .collect(),
            platforms: descriptor.platforms().map(Into::into).collect(),
            architectures: descriptor.architectures().map(Into::into).collect(),
            abi: [descriptor.abi().major(), descriptor.abi().minor()],
        })
    }

    fn into_deployment(self) -> Result<NativeAdapterDeployment, DeploymentError> {
        let descriptor = AdapterDescriptor::new(
            AdapterId::new(self.adapter_id.clone()),
            AdapterVersion::new(self.version[0], self.version[1], self.version[2]),
            self.apply_model.into(),
            self.placement.into(),
            self.descriptor_features.into_iter().map(Into::into),
        )
        .with_platforms(self.platforms)
        .with_architectures(self.architectures)
        .with_abi(AbiVersion::new(self.abi[0], self.abi[1]));
        let host = match descriptor.placement() {
            Placement::TargetProcess => AdapterHostBinding::TargetProcess {
                library: PackageArtifactId::new(self.artifact_id),
            },
            Placement::IsolatedWorker => return Err(DeploymentError::UnsupportedBinding),
        };
        let binding = AdapterBinding {
            adapter_id: descriptor.adapter_id().clone(),
            version: descriptor.version(),
            apply_model: descriptor.apply_model(),
            artifact_hash: ArtifactHash::sha256(self.artifact_sha256),
            descriptor,
            host,
            features: self
                .requested_features
                .into_iter()
                .map(Into::into)
                .collect(),
        };
        NativeAdapterDeployment::new(self.library, binding)
    }
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum WireApplyModel {
    InlineRender,
    RetainedObject,
    ExternalProtocol,
    ObserveOnly,
}

impl From<ApplyModel> for WireApplyModel {
    fn from(value: ApplyModel) -> Self {
        match value {
            ApplyModel::InlineRender => Self::InlineRender,
            ApplyModel::RetainedObject => Self::RetainedObject,
            ApplyModel::ExternalProtocol => Self::ExternalProtocol,
            ApplyModel::ObserveOnly => Self::ObserveOnly,
        }
    }
}

impl From<WireApplyModel> for ApplyModel {
    fn from(value: WireApplyModel) -> Self {
        match value {
            WireApplyModel::InlineRender => Self::InlineRender,
            WireApplyModel::RetainedObject => Self::RetainedObject,
            WireApplyModel::ExternalProtocol => Self::ExternalProtocol,
            WireApplyModel::ObserveOnly => Self::ObserveOnly,
        }
    }
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum WirePlacement {
    TargetProcess,
    IsolatedWorker,
}

impl From<Placement> for WirePlacement {
    fn from(value: Placement) -> Self {
        match value {
            Placement::TargetProcess => Self::TargetProcess,
            Placement::IsolatedWorker => Self::IsolatedWorker,
        }
    }
}

impl From<WirePlacement> for Placement {
    fn from(value: WirePlacement) -> Self {
        match value {
            WirePlacement::TargetProcess => Self::TargetProcess,
            WirePlacement::IsolatedWorker => Self::IsolatedWorker,
        }
    }
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum WireFeature {
    TextObserve,
    TextReplace,
    FontSubstitute,
    LayoutAdjust,
    ResourceReplace,
}

impl From<Feature> for WireFeature {
    fn from(value: Feature) -> Self {
        match value {
            Feature::TextObserve => Self::TextObserve,
            Feature::TextReplace => Self::TextReplace,
            Feature::FontSubstitute => Self::FontSubstitute,
            Feature::LayoutAdjust => Self::LayoutAdjust,
            Feature::ResourceReplace => Self::ResourceReplace,
        }
    }
}

impl From<WireFeature> for Feature {
    fn from(value: WireFeature) -> Self {
        match value {
            WireFeature::TextObserve => Self::TextObserve,
            WireFeature::TextReplace => Self::TextReplace,
            WireFeature::FontSubstitute => Self::FontSubstitute,
            WireFeature::LayoutAdjust => Self::LayoutAdjust,
            WireFeature::ResourceReplace => Self::ResourceReplace,
        }
    }
}

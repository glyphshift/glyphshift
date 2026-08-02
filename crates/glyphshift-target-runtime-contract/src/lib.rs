//! Versioned deployment contract consumed by the injected target Runtime Host.

use glyphshift_adapter_registry::{
    AdapterBinding, AdapterHostBinding, ArtifactHash, PackageArtifactId,
};
use glyphshift_adapter_sdk::{AdapterDescriptor, AdapterVersion};
use glyphshift_capture::{CaptureConfiguration, CaptureSessionId};
use glyphshift_domain::{AbiVersion, AdapterId, ApplyModel, Feature, Placement};
use glyphshift_runtime_contract::{RuntimePublication, RuntimeWireError};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const DEPLOYMENT_SCHEMA: &str = "glyphshift.target-runtime/2";
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

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RuntimeCommandV1 {
    pub struct_size: u32,
    pub json: *const u8,
    pub json_len: u32,
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
        }
    }

    #[must_use]
    pub fn with_capture(mut self, capture: CaptureConfiguration) -> Self {
        self.capture = Some(capture);
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
struct WireDeployment {
    schema: Box<str>,
    publication: String,
    adapters: Vec<WireAdapterDeployment>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    capture: Option<WireCapture>,
}

#[derive(Serialize, Deserialize)]
struct WireCapture {
    session_id: Box<str>,
    output_path: PathBuf,
    max_entries: u32,
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

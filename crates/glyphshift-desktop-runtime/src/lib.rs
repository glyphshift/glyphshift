//! Desktop composition root for a verified Controller, target Runtime, and Adapter bundle.
//!
//! The desktop shell sees only application instances and session state. Process tokens, artifact
//! paths, deployment payloads, and controller acknowledgements stay behind this module boundary.

use glyphshift_adapter_native_host::LoadedNativeAdapter;
use glyphshift_adapter_registry::{
    AdapterPackage, AdapterPackageSet, AdapterRegistry, AdapterRequirement, AdapterTrustPolicy,
    AdapterVersionRequirement, ArtifactHash, PackageArtifactId, SignerId,
};
use glyphshift_capture::CaptureConfiguration;
use glyphshift_controller_host::{
    ControllerStartupConfig, ControllerTrustPolicy, ProcessControllerTransport,
    VerifiedControllerArtifact,
};
use glyphshift_desktop_backend::{DesktopRuntimeSpec, EffectiveWorkflowIntent};
use glyphshift_domain::{Feature, Generation, TargetFacts};
use glyphshift_extension::{
    CodeHash, ControllerArtifactId, ControllerCodeIdentity, ControllerSignerId, ExtensionId,
    ProtocolVersion,
};
use glyphshift_protocol::{
    ControllerConnection, ControllerNonce, ControllerProtocolError, ControllerTransport,
    NonceLedger, OpaqueTargetId, PreparedRecipe, RecipeControllerLossPolicy,
};
use glyphshift_session::{
    ControllerFailure, ControllerHealth, ControllerLossPolicy, ControllerRecipePort, HostFailure,
    SessionError, SessionId, SessionManager, SessionRecipe, TargetHealth, TargetInstance,
    TargetInstanceId, TargetLifecyclePort,
};
pub use glyphshift_session::{
    HostOperationFailure, RuntimeFontOutcome, RuntimeTextOutcome, RuntimeTraceBatch,
    RuntimeTraceRecord, RuntimeTraceStatus,
};
use glyphshift_target_process_host::{RuntimeArtifact, TargetArtifactCatalog, TargetProcessHost};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const BUNDLE_SCHEMA: &str = "glyphshift.runtime-bundle/2";
const FIRST_PARTY_BUNDLE_AUTHORITY: &str = "app.glyphshift.runtime.first-party";
const CONTROLLER_TIMEOUT: Duration = Duration::from_secs(8);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DesktopRuntimeError {
    BundleUnavailable,
    InvalidManifest,
    InvalidArtifactPath,
    InvalidArtifactHash,
    ArtifactHashMismatch,
    AdapterInspectionFailed,
    AdapterRegistryRejected,
    ControllerRejected,
    ControllerUnavailable,
    ProtocolRejected,
    UnknownTarget,
    InvalidState,
    SessionRejected,
    ActivationRejected(HostOperationFailure),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeTarget {
    id: u64,
    display_name: Box<str>,
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

#[derive(Deserialize)]
struct BundleManifest {
    schema: Box<str>,
    authority: Box<str>,
    controller: ControllerManifest,
    runtime: ArtifactManifest,
    adapters: Vec<ArtifactManifest>,
}

#[derive(Deserialize)]
struct ControllerManifest {
    artifact: Box<str>,
    file: Box<str>,
    sha256: Box<str>,
    protocol: [u16; 2],
}

#[derive(Deserialize)]
struct ArtifactManifest {
    file: Box<str>,
    sha256: Box<str>,
    #[serde(default)]
    name: Option<Box<str>>,
    #[serde(default)]
    summary: Option<Box<str>>,
    #[serde(default)]
    technology: Option<Box<str>>,
    #[serde(default)]
    technical_target: Option<Box<str>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeAdapterOption {
    id: Box<str>,
    name: Box<str>,
    version: Box<str>,
    summary: Box<str>,
    platforms: Vec<Box<str>>,
    technologies: Vec<Box<str>>,
    features: Vec<Feature>,
    technical_target: Box<str>,
    configuration: Box<str>,
}

impl RuntimeAdapterOption {
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }

    #[must_use]
    pub fn summary(&self) -> &str {
        &self.summary
    }

    #[must_use]
    pub fn platforms(&self) -> &[Box<str>] {
        &self.platforms
    }

    #[must_use]
    pub fn technologies(&self) -> &[Box<str>] {
        &self.technologies
    }

    #[must_use]
    pub fn features(&self) -> &[Feature] {
        &self.features
    }

    #[must_use]
    pub fn technical_target(&self) -> &str {
        &self.technical_target
    }

    #[must_use]
    pub fn configuration(&self) -> &str {
        &self.configuration
    }
}

/// A verified, path-private set of production runtime artifacts.
pub struct RuntimeBundle {
    controller: VerifiedControllerArtifact,
    controller_protocol: ProtocolVersion,
    registry: AdapterRegistry,
    artifacts: TargetArtifactCatalog,
    discovered_requirements: Vec<AdapterRequirement>,
    translation_adapters: Vec<RuntimeAdapterOption>,
    nonce_ledger: NonceLedger,
    nonce_sequence: u64,
}

impl RuntimeBundle {
    pub fn open(root: impl AsRef<Path>) -> Result<Self, DesktopRuntimeError> {
        let root = root
            .as_ref()
            .canonicalize()
            .map_err(|_| DesktopRuntimeError::BundleUnavailable)?;
        let manifest: BundleManifest = serde_json::from_str(
            &fs::read_to_string(root.join("runtime-bundle.json"))
                .map_err(|_| DesktopRuntimeError::BundleUnavailable)?,
        )
        .map_err(|_| DesktopRuntimeError::InvalidManifest)?;
        if manifest.schema.as_ref() != BUNDLE_SCHEMA
            || manifest.authority.as_ref() != FIRST_PARTY_BUNDLE_AUTHORITY
            || manifest.controller.artifact.trim().is_empty()
            || manifest.adapters.is_empty()
        {
            return Err(DesktopRuntimeError::InvalidManifest);
        }

        let signer = SignerId::new(manifest.authority.clone());
        let controller_signer = ControllerSignerId::new(manifest.authority.clone());
        let controller_hash = parse_hash(&manifest.controller.sha256)?;
        let controller_path = artifact_path(&root, &manifest.controller.file)?;
        let controller_protocol = ProtocolVersion::new(
            manifest.controller.protocol[0],
            manifest.controller.protocol[1],
        );
        let controller_identity = ControllerCodeIdentity::new(
            ControllerArtifactId::new(manifest.controller.artifact),
            controller_signer,
            CodeHash::new(controller_hash),
            controller_protocol,
        );
        let controller = VerifiedControllerArtifact::verify(
            controller_path,
            &controller_identity,
            &ControllerTrustPolicy::new([manifest.authority.clone()]),
        )
        .map_err(|_| DesktopRuntimeError::ControllerRejected)?;

        let runtime_hash = parse_hash(&manifest.runtime.sha256)?;
        let runtime_path = verified_artifact(&root, &manifest.runtime.file, runtime_hash)?;
        let runtime_artifact = RuntimeArtifact::new(runtime_path, runtime_hash);

        let mut packages = Vec::new();
        let mut catalog_adapters = Vec::new();
        let mut authorized_adapters = Vec::new();
        let mut discovered_requirements = Vec::new();
        let mut translation_adapters = Vec::new();
        for (index, adapter) in manifest.adapters.iter().enumerate() {
            let hash = parse_hash(&adapter.sha256)?;
            let path = verified_artifact(&root, &adapter.file, hash)?;
            // SAFETY: `verified_artifact` measured the exact file against the bundle manifest hash
            // before native code is loaded. The bundle authority is fixed by the product above.
            let descriptor = unsafe { LoadedNativeAdapter::inspect(&path) }
                .map_err(|_| DesktopRuntimeError::AdapterInspectionFailed)?;
            let features = descriptor
                .features()
                .filter(|feature| {
                    matches!(
                        feature,
                        Feature::TextObserve | Feature::TextReplace | Feature::FontSubstitute
                    )
                })
                .collect::<Vec<_>>();
            if features.contains(&Feature::TextReplace) {
                translation_adapters.push(RuntimeAdapterOption {
                    id: descriptor.adapter_id().as_str().into(),
                    name: adapter
                        .name
                        .clone()
                        .unwrap_or_else(|| descriptor.adapter_id().as_str().into()),
                    version: format!(
                        "{}.{}.{}",
                        descriptor.version().major(),
                        descriptor.version().minor(),
                        descriptor.version().patch()
                    )
                    .into(),
                    summary: adapter
                        .summary
                        .clone()
                        .unwrap_or_else(|| "运行时文字与字体拦截适配器".into()),
                    platforms: descriptor.platforms().map(Into::into).collect(),
                    technologies: adapter.technology.iter().cloned().collect(),
                    features: features.clone(),
                    technical_target: adapter
                        .technical_target
                        .clone()
                        .unwrap_or_else(|| descriptor.adapter_id().as_str().into()),
                    configuration: "none".into(),
                });
            }
            let artifact_id = PackageArtifactId::new(format!("adapters/{index}"));
            discovered_requirements.push(AdapterRequirement::new(
                descriptor.adapter_id().clone(),
                AdapterVersionRequirement::Exact(descriptor.version()),
                features,
            ));
            authorized_adapters.push(descriptor.adapter_id().clone());
            packages.push(AdapterPackage::new(
                descriptor,
                artifact_id.clone(),
                signer.clone(),
                ArtifactHash::sha256(hash),
                ArtifactHash::sha256(hash),
            ));
            catalog_adapters.push((artifact_id, path));
        }

        let mut registry =
            AdapterRegistry::new(AdapterTrustPolicy::new([signer], authorized_adapters));
        registry
            .reload(AdapterPackageSet::new(packages))
            .map_err(|_| DesktopRuntimeError::AdapterRegistryRejected)?;
        let artifacts = TargetArtifactCatalog::new(runtime_artifact, catalog_adapters)
            .map_err(|_| DesktopRuntimeError::BundleUnavailable)?;

        Ok(Self {
            controller,
            controller_protocol,
            registry,
            artifacts,
            discovered_requirements,
            translation_adapters,
            nonce_ledger: NonceLedger::new(),
            nonce_sequence: 0,
        })
    }

    #[must_use]
    pub fn translation_adapter_ids(&self) -> Vec<Box<str>> {
        self.discovered_requirements
            .iter()
            .filter(|requirement| {
                requirement
                    .features()
                    .any(|feature| feature == Feature::TextReplace)
            })
            .map(|requirement| requirement.adapter_id().as_str().into())
            .collect()
    }

    #[must_use]
    pub fn translation_adapter_options(&self) -> &[RuntimeAdapterOption] {
        &self.translation_adapters
    }

    #[must_use]
    pub fn adapter_requirements(&self) -> &[AdapterRequirement] {
        &self.discovered_requirements
    }

    pub fn discover(
        &mut self,
        application_id: impl Into<Box<str>>,
        spec: &DesktopRuntimeSpec,
    ) -> Result<WindowsDesktopRuntime, DesktopRuntimeError> {
        let application_id = application_id.into();
        let requirements = if spec.requirements().is_empty() {
            self.discovered_requirements.clone()
        } else {
            spec.requirements().to_vec()
        };
        let transport = ProcessControllerTransport::spawn_configured(
            self.controller.clone(),
            CONTROLLER_TIMEOUT,
            ControllerStartupConfig::new(
                spec.executable_names().iter().cloned(),
                requirements.clone(),
            )
            .with_executable_paths(spec.executable_paths().iter().cloned()),
        )
        .map_err(|_| DesktopRuntimeError::ControllerUnavailable)?;
        self.nonce_sequence = self.nonce_sequence.saturating_add(1);
        DesktopRuntime::connect(
            transport,
            application_id,
            requirements,
            spec.publication().clone(),
            self.registry.clone(),
            self.artifacts.clone(),
            self.controller_protocol,
            next_nonce(self.nonce_sequence),
            &mut self.nonce_ledger,
        )
    }
}

pub type WindowsDesktopRuntime = DesktopRuntime<ProcessControllerTransport>;

trait ManagedRuntime: Send {
    fn application_id(&self) -> &str;
    fn targets(&self) -> Vec<RuntimeTarget>;
    fn supported_features(&self) -> BTreeSet<Feature>;
    fn active_features(&self) -> BTreeSet<Feature>;
    fn active_target_id(&self) -> Option<u64>;
    fn applied_generation(&self) -> Option<Generation>;
    fn start(
        &mut self,
        target_id: u64,
        requested_features: &BTreeSet<Feature>,
    ) -> Result<(), DesktopRuntimeError>;
    fn start_capture(
        &mut self,
        target_id: u64,
        requested_features: &BTreeSet<Feature>,
        capture: CaptureConfiguration,
    ) -> Result<(), DesktopRuntimeError>;
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
        target_id: u64,
        requested_features: &BTreeSet<Feature>,
        capture: CaptureConfiguration,
    ) -> Result<(), DesktopRuntimeError> {
        DesktopRuntime::start_capture(self, target_id, requested_features.iter().copied(), capture)
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

trait RuntimeFactory: Send {
    fn discover(
        &mut self,
        application_id: Box<str>,
        spec: &DesktopRuntimeSpec,
    ) -> Result<Box<dyn ManagedRuntime>, DesktopRuntimeError>;
}

impl RuntimeFactory for RuntimeBundle {
    fn discover(
        &mut self,
        application_id: Box<str>,
        spec: &DesktopRuntimeSpec,
    ) -> Result<Box<dyn ManagedRuntime>, DesktopRuntimeError> {
        RuntimeBundle::discover(self, application_id, spec)
            .map(|runtime| Box::new(runtime) as Box<dyn ManagedRuntime>)
    }
}

/// Path- and process-private desktop state for one registered application.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DesktopRuntimeStatus {
    application_id: Box<str>,
    targets: Vec<RuntimeTarget>,
    active_target_id: Option<u64>,
    supported_features: BTreeSet<Feature>,
    requested_features: BTreeSet<Feature>,
    active_features: BTreeSet<Feature>,
    applied_generation: Option<Generation>,
}

impl DesktopRuntimeStatus {
    #[must_use]
    pub fn application_id(&self) -> &str {
        &self.application_id
    }

    pub fn targets(&self) -> impl Iterator<Item = &RuntimeTarget> {
        self.targets.iter()
    }

    #[must_use]
    pub fn active_target_id(&self) -> Option<u64> {
        self.active_target_id
    }

    #[must_use]
    pub fn supports(&self, feature: Feature) -> bool {
        self.supported_features.contains(&feature)
    }

    #[must_use]
    pub fn is_feature_active(&self, feature: Feature) -> bool {
        self.active_features.contains(&feature)
    }

    #[must_use]
    pub fn is_feature_requested(&self, feature: Feature) -> bool {
        self.requested_features.contains(&feature)
    }

    #[must_use]
    pub fn is_active(&self) -> bool {
        self.active_target_id.is_some() && !self.active_features.is_empty()
    }

    #[must_use]
    pub const fn applied_generation(&self) -> Option<Generation> {
        self.applied_generation
    }

    fn inactive(application_id: Box<str>) -> Self {
        Self {
            application_id,
            targets: Vec::new(),
            active_target_id: None,
            supported_features: BTreeSet::new(),
            requested_features: BTreeSet::new(),
            active_features: BTreeSet::new(),
            applied_generation: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkflowReconcileReport {
    workflow_id: Box<str>,
    statuses: Vec<DesktopRuntimeStatus>,
    errors: BTreeMap<Box<str>, DesktopRuntimeError>,
}

impl WorkflowReconcileReport {
    #[must_use]
    pub fn workflow_id(&self) -> &str {
        &self.workflow_id
    }

    #[must_use]
    pub fn statuses(&self) -> &[DesktopRuntimeStatus] {
        &self.statuses
    }

    #[must_use]
    pub const fn errors(&self) -> &BTreeMap<Box<str>, DesktopRuntimeError> {
        &self.errors
    }
}

/// Owns independent Runtime sessions for every enabled application.
///
/// The desktop shell expresses desired feature state per application. This module owns discovery,
/// target selection, session replacement, publication, and isolated shutdown.
pub struct DesktopRuntimePool {
    factory: Box<dyn RuntimeFactory>,
    sessions: BTreeMap<Box<str>, Box<dyn ManagedRuntime>>,
    requested_features: BTreeMap<Box<str>, BTreeSet<Feature>>,
    workflow_targets: BTreeMap<Box<str>, BTreeSet<Box<str>>>,
    capture_targets: BTreeSet<Box<str>>,
}

impl DesktopRuntimePool {
    #[must_use]
    pub fn new(bundle: RuntimeBundle) -> Self {
        Self::with_factory(Box::new(bundle))
    }

    fn with_factory(factory: Box<dyn RuntimeFactory>) -> Self {
        Self {
            factory,
            sessions: BTreeMap::new(),
            requested_features: BTreeMap::new(),
            workflow_targets: BTreeMap::new(),
            capture_targets: BTreeSet::new(),
        }
    }

    #[must_use]
    pub fn status(&self, application_id: &str) -> Option<DesktopRuntimeStatus> {
        self.sessions.get(application_id).map(|runtime| {
            runtime_status(
                runtime.as_ref(),
                self.requested_features
                    .get(application_id)
                    .cloned()
                    .unwrap_or_default(),
            )
        })
    }

    pub fn statuses(&self) -> impl Iterator<Item = DesktopRuntimeStatus> + '_ {
        self.sessions.iter().map(|(application_id, runtime)| {
            runtime_status(
                runtime.as_ref(),
                self.requested_features
                    .get(application_id)
                    .cloned()
                    .unwrap_or_default(),
            )
        })
    }

    pub fn discover(
        &mut self,
        application_id: impl Into<Box<str>>,
        spec: &DesktopRuntimeSpec,
    ) -> Result<DesktopRuntimeStatus, DesktopRuntimeError> {
        let application_id = application_id.into();
        if !self.sessions.contains_key(application_id.as_ref()) {
            let runtime = self.factory.discover(application_id.clone(), spec)?;
            self.sessions.insert(application_id.clone(), runtime);
        }
        self.status(&application_id)
            .ok_or(DesktopRuntimeError::InvalidState)
    }

    pub fn set_features(
        &mut self,
        application_id: impl Into<Box<str>>,
        spec: &DesktopRuntimeSpec,
        target_id: Option<u64>,
        requested_features: impl IntoIterator<Item = Feature>,
    ) -> Result<DesktopRuntimeStatus, DesktopRuntimeError> {
        let application_id = application_id.into();
        let requested_features = requested_features.into_iter().collect::<BTreeSet<_>>();
        if requested_features.is_empty() {
            return self.stop_application(application_id);
        }

        let must_replace = self
            .sessions
            .get(application_id.as_ref())
            .is_some_and(|runtime| {
                runtime.is_active() && runtime.active_features() != requested_features
            });
        if must_replace {
            let mut runtime = self
                .sessions
                .remove(application_id.as_ref())
                .ok_or(DesktopRuntimeError::InvalidState)?;
            if let Err(error) = runtime.stop() {
                self.sessions.insert(application_id.clone(), runtime);
                return Err(error);
            }
        }

        self.discover(application_id.clone(), spec)?;
        let runtime = self
            .sessions
            .get_mut(application_id.as_ref())
            .ok_or(DesktopRuntimeError::InvalidState)?;
        if runtime.is_active() {
            if runtime
                .applied_generation()
                .is_none_or(|generation| spec.publication().generation() > generation)
            {
                runtime.publish(spec.publication().clone())?;
            }
            self.requested_features
                .insert(application_id.clone(), requested_features.clone());
            return Ok(runtime_status(runtime.as_ref(), requested_features));
        }
        let target_id = target_id
            .or_else(|| runtime.targets().first().map(RuntimeTarget::id))
            .ok_or(DesktopRuntimeError::UnknownTarget)?;
        runtime.start(target_id, &requested_features)?;
        self.requested_features
            .insert(application_id.clone(), runtime.active_features());
        Ok(runtime_status(
            runtime.as_ref(),
            self.requested_features
                .get(application_id.as_ref())
                .cloned()
                .unwrap_or_default(),
        ))
    }

    pub fn reconcile_workflow(
        &mut self,
        intent: &EffectiveWorkflowIntent,
    ) -> Result<WorkflowReconcileReport, DesktopRuntimeError> {
        let target_ids = intent
            .targets()
            .iter()
            .map(|target| Box::<str>::from(target.software_id()))
            .collect::<BTreeSet<_>>();
        if target_ids
            .iter()
            .any(|software_id| self.capture_targets.contains(software_id))
        {
            return Err(DesktopRuntimeError::InvalidState);
        }
        if self.workflow_targets.iter().any(|(workflow_id, owned)| {
            workflow_id.as_ref() != intent.workflow_id()
                && owned
                    .iter()
                    .any(|software_id| target_ids.contains(software_id))
        }) {
            return Err(DesktopRuntimeError::InvalidState);
        }

        let mut statuses = Vec::new();
        let mut errors = BTreeMap::new();
        let previous_targets = self
            .workflow_targets
            .get(intent.workflow_id())
            .cloned()
            .unwrap_or_default();
        for software_id in previous_targets.difference(&target_ids) {
            match self.stop_application(software_id.clone()) {
                Ok(status) => statuses.push(status),
                Err(error) => {
                    errors.insert(software_id.clone(), error);
                }
            }
        }
        for target in intent.targets() {
            let requested_features = target
                .requested_features()
                .iter()
                .copied()
                .collect::<BTreeSet<_>>();
            self.requested_features
                .insert(target.software_id().into(), requested_features.clone());
            match self.set_features(
                target.software_id(),
                target.runtime_spec(),
                None,
                requested_features,
            ) {
                Ok(status) => statuses.push(status),
                Err(error) => {
                    errors.insert(target.software_id().into(), error);
                }
            }
        }
        self.workflow_targets
            .insert(intent.workflow_id().into(), target_ids);
        Ok(WorkflowReconcileReport {
            workflow_id: intent.workflow_id().into(),
            statuses,
            errors,
        })
    }

    pub fn stop_workflow(
        &mut self,
        workflow_id: &str,
    ) -> Result<WorkflowReconcileReport, DesktopRuntimeError> {
        let target_ids = self
            .workflow_targets
            .remove(workflow_id)
            .ok_or(DesktopRuntimeError::InvalidState)?;
        let mut statuses = Vec::new();
        let mut errors = BTreeMap::new();
        for software_id in target_ids {
            match self.stop_application(software_id.clone()) {
                Ok(status) => statuses.push(status),
                Err(error) => {
                    errors.insert(software_id, error);
                }
            }
        }
        Ok(WorkflowReconcileReport {
            workflow_id: workflow_id.into(),
            statuses,
            errors,
        })
    }

    pub fn replace_workflow(
        &mut self,
        intent: &EffectiveWorkflowIntent,
    ) -> Result<WorkflowReconcileReport, DesktopRuntimeError> {
        let replacement_targets = intent
            .targets()
            .iter()
            .map(|target| target.software_id())
            .collect::<BTreeSet<_>>();
        let replaced_workflow_ids = self
            .workflow_targets
            .iter()
            .filter(|(workflow_id, targets)| {
                workflow_id.as_ref() != intent.workflow_id()
                    && targets
                        .iter()
                        .any(|software_id| replacement_targets.contains(software_id.as_ref()))
            })
            .map(|(workflow_id, _)| workflow_id.clone())
            .collect::<Vec<_>>();
        let mut statuses = Vec::new();
        let mut errors = BTreeMap::new();
        for workflow_id in replaced_workflow_ids {
            let stopped = self.stop_workflow(&workflow_id)?;
            statuses.extend(stopped.statuses);
            errors.extend(stopped.errors);
        }
        let reconciled = self.reconcile_workflow(intent)?;
        statuses.extend(reconciled.statuses);
        errors.extend(reconciled.errors);
        Ok(WorkflowReconcileReport {
            workflow_id: intent.workflow_id().into(),
            statuses,
            errors,
        })
    }

    fn stop_application(
        &mut self,
        application_id: Box<str>,
    ) -> Result<DesktopRuntimeStatus, DesktopRuntimeError> {
        let Some(mut runtime) = self.sessions.remove(application_id.as_ref()) else {
            self.requested_features.remove(application_id.as_ref());
            return Ok(DesktopRuntimeStatus::inactive(application_id));
        };
        if runtime.is_active() {
            if let Err(error) = runtime.stop() {
                self.sessions.insert(application_id, runtime);
                return Err(error);
            }
        }
        self.requested_features.remove(application_id.as_ref());
        Ok(runtime_status(runtime.as_ref(), BTreeSet::new()))
    }

    pub fn start_capture(
        &mut self,
        application_id: impl Into<Box<str>>,
        spec: &DesktopRuntimeSpec,
        target_id: Option<u64>,
        capture: CaptureConfiguration,
    ) -> Result<DesktopRuntimeStatus, DesktopRuntimeError> {
        let application_id = application_id.into();
        if self.capture_targets.contains(application_id.as_ref())
            || self
                .workflow_targets
                .values()
                .any(|targets| targets.contains(application_id.as_ref()))
        {
            return Err(DesktopRuntimeError::InvalidState);
        }
        let status = self.discover(application_id.clone(), spec)?;
        if !status.supports(Feature::TextObserve) || status.is_active() {
            return Err(DesktopRuntimeError::SessionRejected);
        }
        let runtime = self
            .sessions
            .get_mut(application_id.as_ref())
            .ok_or(DesktopRuntimeError::InvalidState)?;
        let target_id = target_id
            .or_else(|| runtime.targets().first().map(RuntimeTarget::id))
            .ok_or(DesktopRuntimeError::UnknownTarget)?;
        let mut requested_features = BTreeSet::from([Feature::TextObserve]);
        if status.supports(Feature::TextReplace) {
            requested_features.insert(Feature::TextReplace);
        }
        if let Err(error) = runtime.start_capture(target_id, &requested_features, capture) {
            if let Some(mut failed) = self.sessions.remove(application_id.as_ref()) {
                failed.abandon();
            }
            self.requested_features.remove(application_id.as_ref());
            return Err(error);
        }
        let runtime = self
            .sessions
            .get(application_id.as_ref())
            .ok_or(DesktopRuntimeError::InvalidState)?;
        self.requested_features
            .insert(application_id.clone(), requested_features.clone());
        self.capture_targets.insert(application_id.clone());
        Ok(runtime_status(runtime.as_ref(), requested_features))
    }

    pub fn stop_capture(
        &mut self,
        application_id: impl Into<Box<str>>,
    ) -> Result<DesktopRuntimeStatus, DesktopRuntimeError> {
        let application_id = application_id.into();
        if !self.capture_targets.remove(application_id.as_ref()) {
            return Err(DesktopRuntimeError::InvalidState);
        }
        self.stop_application(application_id)
    }

    pub fn control_capture(
        &mut self,
        application_id: &str,
        paused: bool,
    ) -> Result<(), DesktopRuntimeError> {
        if !self.capture_targets.contains(application_id) {
            return Err(DesktopRuntimeError::InvalidState);
        }
        self.sessions
            .get_mut(application_id)
            .ok_or(DesktopRuntimeError::InvalidState)?
            .control_capture(paused)
    }

    pub fn control_runtime_diagnostics(
        &mut self,
        application_id: &str,
        enabled: bool,
    ) -> Result<(), DesktopRuntimeError> {
        let runtime = self
            .sessions
            .get_mut(application_id)
            .ok_or(DesktopRuntimeError::InvalidState)?;
        if !runtime.is_active() {
            return Err(DesktopRuntimeError::InvalidState);
        }
        runtime.control_runtime_diagnostics(enabled)
    }

    pub fn query_runtime_diagnostics(
        &mut self,
        application_id: &str,
    ) -> Result<RuntimeTraceBatch, DesktopRuntimeError> {
        let runtime = self
            .sessions
            .get_mut(application_id)
            .ok_or(DesktopRuntimeError::InvalidState)?;
        if !runtime.is_active() {
            return Err(DesktopRuntimeError::InvalidState);
        }
        runtime.query_runtime_diagnostics()
    }

    pub fn refresh(
        &mut self,
        application_id: impl Into<Box<str>>,
        spec: &DesktopRuntimeSpec,
    ) -> Result<DesktopRuntimeStatus, DesktopRuntimeError> {
        let application_id = application_id.into();
        let requested_features = self
            .requested_features
            .get(application_id.as_ref())
            .cloned()
            .unwrap_or_default();
        if let Some(mut runtime) = self.sessions.remove(application_id.as_ref()) {
            if runtime.is_active() && runtime.stop().is_err() {
                runtime.abandon();
            }
        }

        let mut runtime = self.factory.discover(application_id.clone(), spec)?;
        if !requested_features.is_empty() {
            let target_id = runtime.targets().first().map(RuntimeTarget::id);
            if let Some(target_id) = target_id {
                runtime.start(target_id, &requested_features)?;
            }
        }
        let status = runtime_status(runtime.as_ref(), requested_features);
        self.sessions.insert(application_id, runtime);
        Ok(status)
    }

    pub fn refresh_workflow(
        &mut self,
        intent: &EffectiveWorkflowIntent,
    ) -> Result<WorkflowReconcileReport, DesktopRuntimeError> {
        let owned = self
            .workflow_targets
            .get(intent.workflow_id())
            .ok_or(DesktopRuntimeError::InvalidState)?;
        if intent
            .targets()
            .iter()
            .any(|target| !owned.contains(target.software_id()))
        {
            return Err(DesktopRuntimeError::InvalidState);
        }
        let mut statuses = Vec::new();
        let mut errors = BTreeMap::new();
        for target in intent.targets() {
            match self.refresh(target.software_id(), target.runtime_spec()) {
                Ok(status) => statuses.push(status),
                Err(error) => {
                    errors.insert(target.software_id().into(), error);
                }
            }
        }
        Ok(WorkflowReconcileReport {
            workflow_id: intent.workflow_id().into(),
            statuses,
            errors,
        })
    }

    pub fn publish(
        &mut self,
        application_id: &str,
        publication: glyphshift_runtime_contract::RuntimePublication,
    ) -> Result<(), DesktopRuntimeError> {
        let runtime = self
            .sessions
            .get_mut(application_id)
            .ok_or(DesktopRuntimeError::InvalidState)?;
        runtime.publish(publication)
    }

    pub fn remove(&mut self, application_id: &str) -> Result<(), DesktopRuntimeError> {
        let Some(mut runtime) = self.sessions.remove(application_id) else {
            self.requested_features.remove(application_id);
            return Ok(());
        };
        if runtime.is_active() {
            if let Err(error) = runtime.stop() {
                self.sessions.insert(application_id.into(), runtime);
                return Err(error);
            }
        }
        self.requested_features.remove(application_id);
        self.capture_targets.remove(application_id);
        Ok(())
    }
}

fn runtime_status(
    runtime: &dyn ManagedRuntime,
    requested_features: BTreeSet<Feature>,
) -> DesktopRuntimeStatus {
    DesktopRuntimeStatus {
        application_id: runtime.application_id().into(),
        targets: runtime.targets(),
        active_target_id: runtime.active_target_id(),
        supported_features: runtime.supported_features(),
        requested_features,
        active_features: runtime.active_features(),
        applied_generation: runtime.applied_generation(),
    }
}

enum RuntimePhase<T> {
    Discovered(ControllerConnection<T>),
    Active {
        manager: SessionManager,
        session_id: SessionId,
        target_id: u64,
    },
}

/// One selected application's runtime session. Constructed by [`RuntimeBundle::discover`].
pub struct DesktopRuntime<T> {
    application_id: Box<str>,
    supported_features: BTreeSet<Feature>,
    publication: glyphshift_runtime_contract::RuntimePublication,
    registry: AdapterRegistry,
    artifacts: TargetArtifactCatalog,
    targets: Vec<TargetRecord>,
    active_features: BTreeSet<Feature>,
    phase: Option<RuntimePhase<T>>,
}

impl<T: ControllerTransport + Send + 'static> DesktopRuntime<T> {
    #[allow(clippy::too_many_arguments)]
    fn connect(
        transport: T,
        application_id: Box<str>,
        requirements: Vec<AdapterRequirement>,
        publication: glyphshift_runtime_contract::RuntimePublication,
        registry: AdapterRegistry,
        artifacts: TargetArtifactCatalog,
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
            Some(RuntimePhase::Active { target_id, .. }) => Some(*target_id),
            _ => None,
        }
    }

    pub fn start(
        &mut self,
        target_id: u64,
        requested_features: impl IntoIterator<Item = Feature>,
    ) -> Result<(), DesktopRuntimeError> {
        self.start_inner(target_id, requested_features, None)
    }

    pub fn start_capture(
        &mut self,
        target_id: u64,
        requested_features: impl IntoIterator<Item = Feature>,
        capture: CaptureConfiguration,
    ) -> Result<(), DesktopRuntimeError> {
        self.start_inner(target_id, requested_features, Some(capture))
    }

    fn start_inner(
        &mut self,
        target_id: u64,
        requested_features: impl IntoIterator<Item = Feature>,
        capture: Option<CaptureConfiguration>,
    ) -> Result<(), DesktopRuntimeError> {
        let requested_features = requested_features.into_iter().collect::<Vec<_>>();
        if requested_features.is_empty()
            || requested_features
                .iter()
                .any(|feature| !self.supported_features.contains(feature))
        {
            return Err(DesktopRuntimeError::SessionRejected);
        }
        let target = self
            .targets
            .iter()
            .find(|target| target.view.id == target_id)
            .cloned()
            .ok_or(DesktopRuntimeError::UnknownTarget)?;
        let mut connection = match self.phase.take() {
            Some(RuntimePhase::Discovered(connection)) => connection,
            phase => {
                self.phase = phase;
                return Err(DesktopRuntimeError::InvalidState);
            }
        };
        let prepared = connection
            .prepare(target.controller_id, requested_features.iter().copied())
            .map_err(map_protocol_error)?;
        let target_instance_id = TargetInstanceId::new(format!("target-{target_id}"));
        let target_instance = TargetInstance::new(target_instance_id.clone(), target.facts);
        let mut host = TargetProcessHost::new(connection, self.artifacts.clone());
        if let Some(capture) = capture {
            host = host.with_capture(capture);
        }
        host.register_target(target_instance_id, target.controller_id);
        let mut manager = SessionManager::new(
            self.registry.clone(),
            PreparedController::new(prepared),
            host,
            RunningTarget,
        );
        let active_features = requested_features.iter().copied().collect();
        let status = manager
            .start_with_runtime(target_instance, requested_features, &self.publication)
            .map_err(map_session_runtime_error)?;
        self.phase = Some(RuntimePhase::Active {
            manager,
            session_id: status.session_id(),
            target_id,
        });
        self.active_features = active_features;
        Ok(())
    }

    pub fn publish(
        &mut self,
        publication: glyphshift_runtime_contract::RuntimePublication,
    ) -> Result<(), DesktopRuntimeError> {
        let Some(RuntimePhase::Active {
            manager,
            session_id,
            ..
        }) = self.phase.as_mut()
        else {
            return Err(DesktopRuntimeError::InvalidState);
        };
        manager
            .update_with_runtime(*session_id, &publication)
            .map_err(|_| DesktopRuntimeError::SessionRejected)?;
        self.publication = publication;
        Ok(())
    }

    pub fn control_capture(&mut self, paused: bool) -> Result<(), DesktopRuntimeError> {
        let Some(RuntimePhase::Active {
            manager,
            session_id,
            ..
        }) = self.phase.as_mut()
        else {
            return Err(DesktopRuntimeError::InvalidState);
        };
        manager
            .control_capture(*session_id, paused)
            .map_err(|_| DesktopRuntimeError::SessionRejected)
    }

    pub fn control_runtime_diagnostics(
        &mut self,
        enabled: bool,
    ) -> Result<(), DesktopRuntimeError> {
        let Some(RuntimePhase::Active {
            manager,
            session_id,
            ..
        }) = self.phase.as_mut()
        else {
            return Err(DesktopRuntimeError::InvalidState);
        };
        manager
            .control_runtime_diagnostics(*session_id, enabled)
            .map_err(|_| DesktopRuntimeError::SessionRejected)
    }

    pub fn query_runtime_diagnostics(&mut self) -> Result<RuntimeTraceBatch, DesktopRuntimeError> {
        let Some(RuntimePhase::Active {
            manager,
            session_id,
            ..
        }) = self.phase.as_mut()
        else {
            return Err(DesktopRuntimeError::InvalidState);
        };
        manager
            .query_runtime_diagnostics(*session_id)
            .map_err(|_| DesktopRuntimeError::SessionRejected)
    }

    pub fn stop(&mut self) -> Result<(), DesktopRuntimeError> {
        let Some(RuntimePhase::Active {
            manager,
            session_id,
            ..
        }) = self.phase.as_mut()
        else {
            return Err(DesktopRuntimeError::InvalidState);
        };
        manager
            .stop(*session_id)
            .map_err(|_| DesktopRuntimeError::SessionRejected)?;
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
            session_id,
            ..
        }) = self.phase.as_mut()
        {
            let _ = manager.stop(*session_id);
        }
    }
}

struct PreparedController {
    recipe: SessionRecipe,
}

impl PreparedController {
    fn new(prepared: PreparedRecipe) -> Self {
        let loss_policy = match prepared.controller_loss_policy() {
            RecipeControllerLossPolicy::Continue => ControllerLossPolicy::Continue,
            RecipeControllerLossPolicy::Degrade => ControllerLossPolicy::Degrade,
        };
        Self {
            recipe: SessionRecipe::new(prepared.requirements().iter().cloned(), loss_policy),
        }
    }
}

impl ControllerRecipePort for PreparedController {
    fn prepare(
        &mut self,
        _target: &TargetInstance,
        _requested_features: &BTreeSet<Feature>,
    ) -> Result<SessionRecipe, ControllerFailure> {
        Ok(self.recipe.clone())
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

fn artifact_path(root: &Path, file: &str) -> Result<PathBuf, DesktopRuntimeError> {
    let path = Path::new(file);
    let mut components = path.components();
    if !matches!(components.next(), Some(Component::Normal(_))) || components.next().is_some() {
        return Err(DesktopRuntimeError::InvalidArtifactPath);
    }
    Ok(root.join(path))
}

fn verified_artifact(
    root: &Path,
    file: &str,
    declared_hash: [u8; 32],
) -> Result<PathBuf, DesktopRuntimeError> {
    let path = artifact_path(root, file)?;
    if measure_hash(&path)? != declared_hash {
        return Err(DesktopRuntimeError::ArtifactHashMismatch);
    }
    Ok(path)
}

fn measure_hash(path: &Path) -> Result<[u8; 32], DesktopRuntimeError> {
    let mut file = File::open(path).map_err(|_| DesktopRuntimeError::BundleUnavailable)?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|_| DesktopRuntimeError::BundleUnavailable)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(digest.finalize().into())
}

fn parse_hash(value: &str) -> Result<[u8; 32], DesktopRuntimeError> {
    if value.len() != 64 {
        return Err(DesktopRuntimeError::InvalidArtifactHash);
    }
    let mut hash = [0_u8; 32];
    for (index, byte) in hash.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16)
            .map_err(|_| DesktopRuntimeError::InvalidArtifactHash)?;
    }
    Ok(hash)
}

fn next_nonce(sequence: u64) -> ControllerNonce {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let mut bytes = [0_u8; 32];
    bytes[..8].copy_from_slice(&sequence.to_le_bytes());
    bytes[8..24].copy_from_slice(&timestamp.to_le_bytes());
    let process = u64::from(std::process::id());
    bytes[24..].copy_from_slice(&process.to_le_bytes());
    ControllerNonce::new(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use glyphshift_desktop_backend::{
        DesktopBackend, DesktopEnvironment, DictionaryCreate, DictionaryEdit,
        DictionaryEntryCreate, ExecutableSelection, WorkflowCreate, WorkflowTargetCreate,
    };
    use glyphshift_protocol::{
        ControllerHello, ControllerInventory, ControllerTarget, ControllerTargetToken,
        TransportFailure,
    };
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    use tempfile::tempdir;

    const TEST_ADAPTER_ID: &str = "test.inline";

    fn open_test_backend(root: impl AsRef<Path>) -> DesktopBackend {
        DesktopBackend::open_with_environment(
            root,
            DesktopEnvironment::new(
                [AdapterRequirement::new(
                    glyphshift_domain::AdapterId::new(TEST_ADAPTER_ID),
                    AdapterVersionRequirement::Exact(
                        glyphshift_adapter_registry::AdapterVersion::new(1, 0, 0),
                    ),
                    [
                        Feature::TextObserve,
                        Feature::TextReplace,
                        Feature::FontSubstitute,
                    ],
                )],
                Vec::<Box<str>>::new(),
            ),
        )
        .expect("desktop backend")
    }

    struct InventoryController;

    struct InMemoryRuntimeFactory;

    impl RuntimeFactory for InMemoryRuntimeFactory {
        fn discover(
            &mut self,
            application_id: Box<str>,
            spec: &DesktopRuntimeSpec,
        ) -> Result<Box<dyn ManagedRuntime>, DesktopRuntimeError> {
            Ok(Box::new(InMemoryRuntime {
                stop_fails: application_id.contains("stopfailure"),
                application_id,
                active_features: BTreeSet::new(),
                generation: spec.publication().generation(),
            }))
        }
    }

    struct InMemoryRuntime {
        application_id: Box<str>,
        active_features: BTreeSet<Feature>,
        generation: Generation,
        stop_fails: bool,
    }

    struct RetryRuntimeFactory {
        discoveries: Arc<AtomicUsize>,
    }

    impl RuntimeFactory for RetryRuntimeFactory {
        fn discover(
            &mut self,
            application_id: Box<str>,
            spec: &DesktopRuntimeSpec,
        ) -> Result<Box<dyn ManagedRuntime>, DesktopRuntimeError> {
            let reject_capture = self.discoveries.fetch_add(1, Ordering::SeqCst) == 0;
            Ok(Box::new(RetryRuntime {
                inner: InMemoryRuntime {
                    application_id,
                    active_features: BTreeSet::new(),
                    generation: spec.publication().generation(),
                    stop_fails: false,
                },
                reject_capture,
            }))
        }
    }

    struct RetryRuntime {
        inner: InMemoryRuntime,
        reject_capture: bool,
    }

    impl ManagedRuntime for RetryRuntime {
        fn application_id(&self) -> &str {
            self.inner.application_id()
        }

        fn targets(&self) -> Vec<RuntimeTarget> {
            self.inner.targets()
        }

        fn supported_features(&self) -> BTreeSet<Feature> {
            self.inner.supported_features()
        }

        fn active_features(&self) -> BTreeSet<Feature> {
            self.inner.active_features()
        }

        fn active_target_id(&self) -> Option<u64> {
            self.inner.active_target_id()
        }

        fn applied_generation(&self) -> Option<Generation> {
            self.inner.applied_generation()
        }

        fn start(
            &mut self,
            target_id: u64,
            requested_features: &BTreeSet<Feature>,
        ) -> Result<(), DesktopRuntimeError> {
            self.inner.start(target_id, requested_features)
        }

        fn start_capture(
            &mut self,
            target_id: u64,
            requested_features: &BTreeSet<Feature>,
            capture: CaptureConfiguration,
        ) -> Result<(), DesktopRuntimeError> {
            if self.reject_capture {
                return Err(DesktopRuntimeError::ProtocolRejected);
            }
            self.inner
                .start_capture(target_id, requested_features, capture)
        }

        fn publish(
            &mut self,
            publication: glyphshift_runtime_contract::RuntimePublication,
        ) -> Result<(), DesktopRuntimeError> {
            self.inner.publish(publication)
        }

        fn control_capture(&mut self, paused: bool) -> Result<(), DesktopRuntimeError> {
            self.inner.control_capture(paused)
        }

        fn control_runtime_diagnostics(
            &mut self,
            enabled: bool,
        ) -> Result<(), DesktopRuntimeError> {
            self.inner.control_runtime_diagnostics(enabled)
        }

        fn query_runtime_diagnostics(&mut self) -> Result<RuntimeTraceBatch, DesktopRuntimeError> {
            self.inner.query_runtime_diagnostics()
        }

        fn stop(&mut self) -> Result<(), DesktopRuntimeError> {
            self.inner.stop()
        }
    }

    impl ManagedRuntime for InMemoryRuntime {
        fn application_id(&self) -> &str {
            &self.application_id
        }

        fn targets(&self) -> Vec<RuntimeTarget> {
            vec![RuntimeTarget {
                id: 1,
                display_name: "合成目标".into(),
            }]
        }

        fn supported_features(&self) -> BTreeSet<Feature> {
            [
                Feature::TextObserve,
                Feature::TextReplace,
                Feature::FontSubstitute,
            ]
            .into_iter()
            .collect()
        }

        fn active_features(&self) -> BTreeSet<Feature> {
            self.active_features.clone()
        }

        fn active_target_id(&self) -> Option<u64> {
            (!self.active_features.is_empty()).then_some(1)
        }

        fn applied_generation(&self) -> Option<Generation> {
            (!self.active_features.is_empty()).then_some(self.generation)
        }

        fn start(
            &mut self,
            _target_id: u64,
            requested_features: &BTreeSet<Feature>,
        ) -> Result<(), DesktopRuntimeError> {
            self.active_features = requested_features.clone();
            Ok(())
        }

        fn start_capture(
            &mut self,
            _target_id: u64,
            requested_features: &BTreeSet<Feature>,
            _capture: CaptureConfiguration,
        ) -> Result<(), DesktopRuntimeError> {
            self.active_features = requested_features.clone();
            Ok(())
        }

        fn publish(
            &mut self,
            publication: glyphshift_runtime_contract::RuntimePublication,
        ) -> Result<(), DesktopRuntimeError> {
            self.generation = publication.generation();
            Ok(())
        }

        fn control_capture(&mut self, _paused: bool) -> Result<(), DesktopRuntimeError> {
            Ok(())
        }

        fn control_runtime_diagnostics(
            &mut self,
            _enabled: bool,
        ) -> Result<(), DesktopRuntimeError> {
            Ok(())
        }

        fn query_runtime_diagnostics(&mut self) -> Result<RuntimeTraceBatch, DesktopRuntimeError> {
            Ok(RuntimeTraceBatch::new(
                [RuntimeTraceRecord::new(
                    TEST_ADAPTER_ID,
                    "Open",
                    RuntimeTraceStatus::Matched,
                    RuntimeTextOutcome::Replaced,
                    RuntimeFontOutcome::Protected,
                    self.generation.value(),
                    [1; 32],
                    [2; 32],
                    [3; 32],
                )],
                3,
            ))
        }

        fn stop(&mut self) -> Result<(), DesktopRuntimeError> {
            if self.stop_fails {
                return Err(DesktopRuntimeError::SessionRejected);
            }
            self.active_features.clear();
            Ok(())
        }
    }

    #[test]
    fn workflow_reconcile_runs_two_targets_and_stops_only_its_owned_software() {
        let root = tempdir().expect("workflow Runtime data");
        let mut backend = open_test_backend(root.path().join("data"));
        let mut software_ids = Vec::new();
        for executable_name in ["Alpha.exe", "Beta.exe", "Gamma.exe"] {
            let executable = root.path().join(executable_name);
            fs::write(&executable, b"synthetic executable").expect("selected executable");
            let snapshot = backend
                .add_software(ExecutableSelection::new(executable))
                .expect("registered executable");
            software_ids.push(
                snapshot
                    .selected_software_id()
                    .expect("selected software")
                    .to_owned(),
            );
        }
        backend
            .create_dictionary(
                DictionaryCreate::new("dictionary.shared", "共享词典", "en-US", "zh-CN")
                    .with_entries([DictionaryEntryCreate::new("Open", "打开")]),
            )
            .expect("shared dictionary");
        backend
            .create_workflow(
                WorkflowCreate::new("workflow.group", "双目标工作流").with_targets([
                    WorkflowTargetCreate::new(
                        software_ids[0].as_str(),
                        [TEST_ADAPTER_ID],
                        ["dictionary.shared"],
                    ),
                    WorkflowTargetCreate::new(
                        software_ids[1].as_str(),
                        [TEST_ADAPTER_ID],
                        ["dictionary.shared"],
                    ),
                ]),
            )
            .expect("group workflow");
        backend
            .create_workflow(
                WorkflowCreate::new("workflow.other", "独立工作流").with_targets([
                    WorkflowTargetCreate::new(
                        software_ids[2].as_str(),
                        [TEST_ADAPTER_ID],
                        ["dictionary.shared"],
                    ),
                ]),
            )
            .expect("other workflow");
        let group = backend
            .effective_workflow_intent("workflow.group")
            .expect("compiled group intent");
        let other = backend
            .effective_workflow_intent("workflow.other")
            .expect("compiled other intent");
        let mut pool = DesktopRuntimePool::with_factory(Box::new(InMemoryRuntimeFactory));

        pool.reconcile_workflow(&other)
            .expect("reconcile unrelated workflow");
        let activated = pool
            .reconcile_workflow(&group)
            .expect("reconcile two targets");
        assert!(activated.errors().is_empty());
        for software_id in &software_ids[..2] {
            let status = pool.status(software_id).expect("owned Runtime status");
            assert!(status.is_feature_requested(Feature::TextReplace));
            assert!(status.is_feature_active(Feature::TextReplace));
        }

        let stopped = pool
            .stop_workflow("workflow.group")
            .expect("stop only the group workflow");
        assert!(stopped.errors().is_empty());
        assert!(software_ids[..2]
            .iter()
            .all(|software_id| pool.status(software_id).is_none()));
        assert!(pool
            .status(&software_ids[2])
            .is_some_and(|status| status.is_feature_active(Feature::TextReplace)));
    }

    #[test]
    fn capture_owns_one_software_and_cannot_overlap_a_translation_workflow() {
        let root = tempdir().expect("capture Runtime data");
        let executable = root.path().join("CaptureHost.exe");
        fs::write(&executable, b"synthetic executable").expect("selected executable");
        let mut backend = open_test_backend(root.path().join("data"));
        let software_id = backend
            .add_software(ExecutableSelection::new(executable))
            .expect("registered executable")
            .selected_software_id()
            .expect("selected software")
            .to_owned();
        backend
            .create_dictionary(
                DictionaryCreate::new("dictionary.capture", "捕获冲突词典", "en-US", "zh-CN")
                    .with_entries([DictionaryEntryCreate::new("Open", "打开")]),
            )
            .expect("capture conflict dictionary");
        backend
            .create_workflow(
                WorkflowCreate::new("workflow.capture", "捕获冲突工作流").with_targets([
                    WorkflowTargetCreate::new(
                        software_id.clone(),
                        [TEST_ADAPTER_ID],
                        ["dictionary.capture"],
                    ),
                ]),
            )
            .expect("capture conflict workflow");
        let spec = backend
            .capture_runtime_spec(&software_id, &[Box::<str>::from(TEST_ADAPTER_ID)])
            .expect("capture spec");
        let intent = backend
            .effective_workflow_intent("workflow.capture")
            .expect("workflow intent");
        let configuration = CaptureConfiguration::new(
            glyphshift_capture::CaptureSessionId::new("capture-test").expect("session id"),
            root.path().join("capture.json"),
            100,
        )
        .expect("capture configuration");
        let mut pool = DesktopRuntimePool::with_factory(Box::new(InMemoryRuntimeFactory));

        let active = pool
            .start_capture(software_id.as_str(), &spec, None, configuration)
            .expect("start capture");
        assert!(active.is_feature_active(Feature::TextObserve));
        pool.control_runtime_diagnostics(software_id.as_str(), true)
            .expect("enable desktop runtime diagnostics");
        let diagnostics = pool
            .query_runtime_diagnostics(software_id.as_str())
            .expect("query desktop runtime diagnostics");
        assert_eq!(diagnostics.records().len(), 1);
        assert_eq!(diagnostics.records()[0].source_text(), "Open");
        assert_eq!(diagnostics.dropped(), 3);
        assert_eq!(
            pool.reconcile_workflow(&intent),
            Err(DesktopRuntimeError::InvalidState)
        );

        pool.stop_capture(software_id.as_str())
            .expect("stop capture");
        assert!(pool
            .reconcile_workflow(&intent)
            .expect("start workflow after capture")
            .errors()
            .is_empty());
    }

    #[test]
    fn failed_capture_start_is_discarded_so_connect_and_continue_can_retry() {
        let root = tempdir().expect("capture retry Runtime data");
        let executable = root.path().join("RetryCaptureHost.exe");
        fs::write(&executable, b"synthetic executable").expect("selected executable");
        let mut backend = open_test_backend(root.path().join("data"));
        let software_id = backend
            .add_software(ExecutableSelection::new(executable))
            .expect("registered executable")
            .selected_software_id()
            .expect("selected software")
            .to_owned();
        let spec = backend
            .capture_runtime_spec(&software_id, &[Box::<str>::from(TEST_ADAPTER_ID)])
            .expect("capture spec");
        let configuration = CaptureConfiguration::new(
            glyphshift_capture::CaptureSessionId::new("capture-retry").expect("session id"),
            root.path().join("capture.json"),
            100,
        )
        .expect("capture configuration");
        let discoveries = Arc::new(AtomicUsize::new(0));
        let mut pool = DesktopRuntimePool::with_factory(Box::new(RetryRuntimeFactory {
            discoveries: Arc::clone(&discoveries),
        }));

        assert_eq!(
            pool.start_capture(software_id.as_str(), &spec, None, configuration.clone()),
            Err(DesktopRuntimeError::ProtocolRejected)
        );
        let connected = pool
            .start_capture(software_id.as_str(), &spec, None, configuration)
            .expect("retry should rediscover a clean Runtime");

        assert!(connected.is_feature_active(Feature::TextObserve));
        assert_eq!(discoveries.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn workflow_reconcile_publishes_a_shared_dictionary_generation_to_every_target() {
        let root = tempdir().expect("workflow Runtime data");
        let mut backend = open_test_backend(root.path().join("data"));
        let mut software_ids = Vec::new();
        for executable_name in ["First.exe", "Second.exe"] {
            let executable = root.path().join(executable_name);
            fs::write(&executable, b"synthetic executable").expect("selected executable");
            let snapshot = backend
                .add_software(ExecutableSelection::new(executable))
                .expect("registered executable");
            software_ids.push(
                snapshot
                    .selected_software_id()
                    .expect("selected software")
                    .to_owned(),
            );
        }
        let dictionary = backend
            .create_dictionary(
                DictionaryCreate::new("dictionary.hot", "共享热更新词典", "en-US", "zh-CN")
                    .with_entries([DictionaryEntryCreate::new("Open", "第一次")]),
            )
            .expect("shared dictionary");
        backend
            .create_workflow(
                WorkflowCreate::new("workflow.hot", "热更新工作流").with_targets([
                    WorkflowTargetCreate::new(
                        software_ids[0].as_str(),
                        [TEST_ADAPTER_ID],
                        ["dictionary.hot"],
                    ),
                    WorkflowTargetCreate::new(
                        software_ids[1].as_str(),
                        [TEST_ADAPTER_ID],
                        ["dictionary.hot"],
                    ),
                ]),
            )
            .expect("shared workflow");
        let mut pool = DesktopRuntimePool::with_factory(Box::new(InMemoryRuntimeFactory));
        let initial = backend
            .effective_workflow_intent("workflow.hot")
            .expect("initial intent");
        pool.reconcile_workflow(&initial)
            .expect("initial reconcile");

        backend
            .update_dictionary(
                DictionaryEdit::new(
                    dictionary.id(),
                    dictionary.metadata().name(),
                    dictionary.metadata().source_locale(),
                    dictionary.metadata().target_locale(),
                    dictionary.revision(),
                )
                .with_entries([DictionaryEntryCreate::new("Open", "第二次")]),
            )
            .expect("update shared dictionary");
        let next = backend
            .effective_workflow_intent("workflow.hot")
            .expect("next intent");
        let updated = pool
            .reconcile_workflow(&next)
            .expect("publish next generation");

        assert_eq!(updated.statuses().len(), 2);
        assert!(updated.statuses().iter().all(|status| {
            status.applied_generation() == Some(glyphshift_domain::Generation::new(3))
        }));
    }

    #[test]
    fn explicit_workflow_replacement_removes_the_entire_old_intent() {
        let root = tempdir().expect("workflow Runtime data");
        let mut backend = open_test_backend(root.path().join("data"));
        let mut software_ids = Vec::new();
        for executable_name in ["OldPrimary.exe", "OldSecondary.exe"] {
            let executable = root.path().join(executable_name);
            fs::write(&executable, b"synthetic executable").expect("selected executable");
            let snapshot = backend
                .add_software(ExecutableSelection::new(executable))
                .expect("registered executable");
            software_ids.push(
                snapshot
                    .selected_software_id()
                    .expect("selected software")
                    .to_owned(),
            );
        }
        backend
            .create_dictionary(
                DictionaryCreate::new("dictionary.replace", "替换词典", "en-US", "zh-CN")
                    .with_entries([DictionaryEntryCreate::new("Open", "打开")]),
            )
            .expect("replacement dictionary");
        backend
            .create_workflow(
                WorkflowCreate::new("workflow.old", "旧工作流").with_targets([
                    WorkflowTargetCreate::new(
                        software_ids[0].as_str(),
                        [TEST_ADAPTER_ID],
                        ["dictionary.replace"],
                    ),
                    WorkflowTargetCreate::new(
                        software_ids[1].as_str(),
                        [TEST_ADAPTER_ID],
                        ["dictionary.replace"],
                    ),
                ]),
            )
            .expect("old workflow");
        backend
            .create_workflow(
                WorkflowCreate::new("workflow.new", "新工作流").with_targets([
                    WorkflowTargetCreate::new(
                        software_ids[0].as_str(),
                        [TEST_ADAPTER_ID],
                        ["dictionary.replace"],
                    ),
                ]),
            )
            .expect("new workflow");
        let old = backend
            .effective_workflow_intent("workflow.old")
            .expect("old intent");
        let new = backend
            .effective_workflow_intent("workflow.new")
            .expect("new intent");
        let mut pool = DesktopRuntimePool::with_factory(Box::new(InMemoryRuntimeFactory));
        pool.reconcile_workflow(&old).expect("start old workflow");

        let replaced = pool
            .replace_workflow(&new)
            .expect("replace the complete old intent");

        assert!(replaced.errors().is_empty());
        assert!(pool
            .status(&software_ids[0])
            .is_some_and(|status| status.is_feature_active(Feature::TextReplace)));
        assert!(pool.status(&software_ids[1]).is_none());
        assert!(pool.stop_workflow("workflow.old").is_err());
    }

    #[test]
    fn workflow_refresh_reapplies_requested_features_after_target_restart() {
        let root = tempdir().expect("workflow Runtime data");
        let executable = root.path().join("Restarted.exe");
        fs::write(&executable, b"synthetic executable").expect("selected executable");
        let mut backend = open_test_backend(root.path().join("data"));
        let snapshot = backend
            .add_software(ExecutableSelection::new(executable))
            .expect("registered executable");
        let software_id = snapshot
            .selected_software_id()
            .expect("selected software")
            .to_owned();
        backend
            .create_dictionary(
                DictionaryCreate::new("dictionary.restart", "重启词典", "en-US", "zh-CN")
                    .with_entries([DictionaryEntryCreate::new("Open", "重连")]),
            )
            .expect("restart dictionary");
        backend
            .create_workflow(
                WorkflowCreate::new("workflow.restart", "重启工作流").with_targets([
                    WorkflowTargetCreate::new(
                        software_id.as_str(),
                        [TEST_ADAPTER_ID],
                        ["dictionary.restart"],
                    ),
                ]),
            )
            .expect("restart workflow");
        let intent = backend
            .effective_workflow_intent("workflow.restart")
            .expect("restart intent");
        let mut pool = DesktopRuntimePool::with_factory(Box::new(InMemoryRuntimeFactory));
        pool.reconcile_workflow(&intent).expect("initial reconcile");

        let refreshed = pool
            .refresh_workflow(&intent)
            .expect("refresh restarted target");

        assert!(refreshed.errors().is_empty());
        let status = pool.status(&software_id).expect("refreshed status");
        assert!(status.is_feature_requested(Feature::TextReplace));
        assert!(status.is_feature_active(Feature::TextReplace));
        assert_eq!(status.applied_generation(), Some(Generation::new(2)));
    }

    #[test]
    fn workflow_stop_failure_keeps_one_last_applied_target_without_rolling_back_others() {
        let root = tempdir().expect("workflow Runtime data");
        let mut backend = open_test_backend(root.path().join("data"));
        let mut software_ids = Vec::new();
        for executable_name in ["StopFailure.exe", "StopSuccess.exe"] {
            let executable = root.path().join(executable_name);
            fs::write(&executable, b"synthetic executable").expect("selected executable");
            let snapshot = backend
                .add_software(ExecutableSelection::new(executable))
                .expect("registered executable");
            software_ids.push(
                snapshot
                    .selected_software_id()
                    .expect("selected software")
                    .to_owned(),
            );
        }
        backend
            .create_dictionary(
                DictionaryCreate::new("dictionary.stop", "停止词典", "en-US", "zh-CN")
                    .with_entries([DictionaryEntryCreate::new("Open", "停止")]),
            )
            .expect("stop dictionary");
        backend
            .create_workflow(
                WorkflowCreate::new("workflow.stop", "停止工作流").with_targets([
                    WorkflowTargetCreate::new(
                        software_ids[0].as_str(),
                        [TEST_ADAPTER_ID],
                        ["dictionary.stop"],
                    ),
                    WorkflowTargetCreate::new(
                        software_ids[1].as_str(),
                        [TEST_ADAPTER_ID],
                        ["dictionary.stop"],
                    ),
                ]),
            )
            .expect("stop workflow");
        let intent = backend
            .effective_workflow_intent("workflow.stop")
            .expect("stop intent");
        let mut pool = DesktopRuntimePool::with_factory(Box::new(InMemoryRuntimeFactory));
        pool.reconcile_workflow(&intent).expect("initial reconcile");

        let stopped = pool
            .stop_workflow("workflow.stop")
            .expect("bounded stop report");

        assert_eq!(
            stopped.errors().get(software_ids[0].as_str()),
            Some(&DesktopRuntimeError::SessionRejected)
        );
        assert!(pool
            .status(&software_ids[0])
            .is_some_and(|status| status.is_feature_active(Feature::TextReplace)));
        assert!(pool.status(&software_ids[1]).is_none());
    }

    impl ControllerTransport for InventoryController {
        fn handshake(
            &mut self,
            expected_extension: &ExtensionId,
            version: ProtocolVersion,
            nonce: ControllerNonce,
        ) -> Result<ControllerHello, TransportFailure> {
            Ok(ControllerHello::new(
                expected_extension.clone(),
                version,
                nonce,
            ))
        }

        fn inventory(&mut self) -> Result<ControllerInventory, TransportFailure> {
            Ok(ControllerInventory::new(
                [],
                [ControllerTarget::new(
                    ControllerTargetToken::new("private-process-token"),
                    "MotionCanvas — 主窗口",
                    TargetFacts::new("windows", "x86_64"),
                )],
            ))
        }

        fn terminate(&mut self) {}
    }

    #[test]
    fn rejects_a_manifest_that_self_authorizes_an_unknown_bundle_authority() {
        let root = tempdir().expect("runtime bundle root");
        fs::write(
            root.path().join("runtime-bundle.json"),
            r#"{
              "schema":"glyphshift.runtime-bundle/2",
              "authority":"example.untrusted",
              "controller":{"artifact":"windows","file":"controller.exe","sha256":"0000000000000000000000000000000000000000000000000000000000000000","protocol":[1,0]},
              "runtime":{"file":"runtime.dll","sha256":"0000000000000000000000000000000000000000000000000000000000000000"},
              "adapters":[{"file":"adapter.dll","sha256":"0000000000000000000000000000000000000000000000000000000000000000"}]
            }"#,
        )
        .expect("bundle manifest");

        assert_eq!(
            RuntimeBundle::open(root.path()).err(),
            Some(DesktopRuntimeError::InvalidManifest)
        );
    }

    #[test]
    fn rejects_parent_paths_before_loading_native_code() {
        let root = tempdir().expect("runtime bundle root");
        fs::write(
            root.path().join("runtime-bundle.json"),
            r#"{
              "schema":"glyphshift.runtime-bundle/2",
              "authority":"app.glyphshift.runtime.first-party",
              "controller":{"artifact":"windows","file":"../controller.exe","sha256":"0000000000000000000000000000000000000000000000000000000000000000","protocol":[1,0]},
              "runtime":{"file":"runtime.dll","sha256":"0000000000000000000000000000000000000000000000000000000000000000"},
              "adapters":[{"file":"adapter.dll","sha256":"0000000000000000000000000000000000000000000000000000000000000000"}]
            }"#,
        )
        .expect("bundle manifest");

        assert_eq!(
            RuntimeBundle::open(root.path()).err(),
            Some(DesktopRuntimeError::InvalidArtifactPath)
        );
    }

    #[test]
    fn rejects_runtime_bytes_that_do_not_match_the_manifest_before_loading_adapters() {
        let root = tempdir().expect("runtime bundle root");
        let controller_bytes = b"synthetic controller";
        fs::write(root.path().join("controller.exe"), controller_bytes)
            .expect("controller artifact");
        fs::write(root.path().join("runtime.dll"), b"changed runtime").expect("runtime artifact");
        fs::write(root.path().join("adapter.dll"), b"unreached adapter").expect("adapter artifact");
        let controller_hash = Sha256::digest(controller_bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        fs::write(
            root.path().join("runtime-bundle.json"),
            format!(
                r#"{{
                  "schema":"glyphshift.runtime-bundle/2",
                  "authority":"app.glyphshift.runtime.first-party",
                  "controller":{{"artifact":"windows","file":"controller.exe","sha256":"{controller_hash}","protocol":[1,0]}},
                  "runtime":{{"file":"runtime.dll","sha256":"0000000000000000000000000000000000000000000000000000000000000000"}},
                  "adapters":[{{"file":"adapter.dll","sha256":"0000000000000000000000000000000000000000000000000000000000000000"}}]
                }}"#
            ),
        )
        .expect("bundle manifest");

        assert_eq!(
            RuntimeBundle::open(root.path()).err(),
            Some(DesktopRuntimeError::ArtifactHashMismatch)
        );
    }

    #[test]
    fn parses_only_full_sha256_values() {
        assert_eq!(
            parse_hash("not-a-hash"),
            Err(DesktopRuntimeError::InvalidArtifactHash)
        );
        assert_eq!(parse_hash(&"f".repeat(64)), Ok([0xff; 32]));
    }

    #[test]
    fn desktop_contract_exposes_instances_without_controller_tokens_or_paths() {
        let root = tempdir().expect("desktop data");
        let executable = root.path().join("MotionCanvas.exe");
        fs::write(&executable, b"synthetic executable").expect("selected executable");
        let mut backend = open_test_backend(root.path().join("data"));
        let snapshot = backend
            .add_software(ExecutableSelection::new(executable))
            .expect("registered executable");
        let application_id = snapshot.software()[0].id();
        let spec = backend
            .runtime_spec(application_id)
            .expect("generic runtime spec");

        let runtime_library = root.path().join("runtime.dll");
        fs::write(&runtime_library, b"synthetic runtime").expect("runtime artifact");
        let artifacts =
            TargetArtifactCatalog::new(RuntimeArtifact::new(&runtime_library, [0; 32]), [])
                .expect("artifact catalog");
        let mut ledger = NonceLedger::new();
        let runtime = DesktopRuntime::connect(
            InventoryController,
            application_id.into(),
            Vec::new(),
            spec.publication().clone(),
            AdapterRegistry::new(AdapterTrustPolicy::new([], [])),
            artifacts,
            ProtocolVersion::new(1, 0),
            ControllerNonce::new([7; 32]),
            &mut ledger,
        )
        .expect("desktop runtime discovery");

        assert_eq!(runtime.application_id(), application_id);
        assert_eq!(
            runtime
                .targets()
                .map(|target| (target.id(), target.display_name()))
                .collect::<Vec<_>>(),
            vec![(1, "MotionCanvas — 主窗口")]
        );
        assert!(!runtime.is_active());
        assert!(!runtime.supports(Feature::TextReplace));
    }
}

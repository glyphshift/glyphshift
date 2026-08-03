mod command_error;
mod settings;

use command_error::CommandError;
use glyphshift_capture::{
    CaptureConfiguration, ProbeDictionaryEntry, ProbeDictionarySnapshot, ProbeEntryPage,
    ProbeExportFormat, ProbeQuery, ProbeRunCreate, ProbeRunError, ProbeRunStatus, ProbeRunStore,
    ProbeRunSummary, DEFAULT_MAX_ENTRIES,
};
use glyphshift_desktop_backend::{
    BackendError, DesktopBackend, DesktopEnvironment, DesktopSnapshot, DictionaryCreate,
    DictionaryEdit, DictionaryEntryCreate, DictionaryView, EffectiveWorkflowIntent,
    ExecutableSelection, SoftwareEdit, WorkflowCreate, WorkflowEdit, WorkflowView,
};
use glyphshift_desktop_runtime::{
    DesktopRuntimeError, DesktopRuntimePool, DesktopRuntimeStatus, HostOperationFailure,
    RuntimeBundle, RuntimeTraceBatch, RuntimeTraceRecord, WorkflowReconcileReport,
};
use glyphshift_dictionary_distribution::{
    ArtifactStatement, ArtifactTrustVerifier, CatalogPage, CatalogPortError, CatalogQuery,
    CatalogRelease, CatalogSourcePage, DictionaryDistribution, DictionaryDistributionError,
    DictionaryDistributionPort, DictionaryReleaseKey, DictionaryReplacementPolicy,
    FileDictionaryInstallStore, InstallRequest, PublisherIdentity, SignatureEnvelope,
    SystemInstallationClock, TrustVerifierError,
};
use glyphshift_domain::{Feature, Generation, RouteOperator};
use glyphshift_runtime_contract::RuntimePublication;
use glyphshift_translation::{FontPolicy, TranslationSnapshot};
use glyphshift_workflow::ResolveError;
use serde::{Deserialize, Serialize};
use settings::{AppSettings, AppSettingsStore, AppSettingsUpdate, SettingsError};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{Manager, State};

const DESKTOP_API_VERSION: u16 = 14;
const WINDOWS_FONT_REGISTRY_KEY: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Fonts";

fn workflow_activation_command_error(error: BackendError) -> CommandError {
    match error {
        BackendError::WorkflowRejected(ResolveError::NoEffectiveRules { software_id }) => {
            CommandError::new("workflow.no_effective_rules")
                .with_arg("softwareId", software_id.to_string())
        }
        BackendError::WorkflowRejected(ResolveError::EmptyAdapterPlan { software_id }) => {
            CommandError::new("workflow.empty_adapter_plan")
                .with_arg("softwareId", software_id.to_string())
        }
        BackendError::WorkflowRejected(ResolveError::LocaleMismatch {
            software_id,
            dictionary_id,
        }) => CommandError::new("workflow.locale_mismatch")
            .with_arg("softwareId", software_id.to_string())
            .with_arg("dictionaryId", dictionary_id.to_string()),
        BackendError::WorkflowRejected(ResolveError::UnknownSoftware(id))
        | BackendError::UnknownSoftware(id) => {
            CommandError::new("workflow.unknown_software").with_arg("softwareId", id.to_string())
        }
        BackendError::WorkflowRejected(ResolveError::UnknownDictionary(id))
        | BackendError::UnknownDictionary(id) => CommandError::new("workflow.unknown_dictionary")
            .with_arg("dictionaryId", id.to_string()),
        BackendError::WorkflowRejected(ResolveError::UnknownAdapter(id)) => {
            CommandError::new("workflow.unknown_adapter").with_arg("adapterId", id.to_string())
        }
        BackendError::WorkflowRejected(ResolveError::EmptyFontFamilies { software_id }) => {
            CommandError::new("workflow.empty_font_families")
                .with_arg("softwareId", software_id.to_string())
        }
        BackendError::WorkflowRejected(ResolveError::FontUnavailable { software_id }) => {
            CommandError::new("workflow.font_unavailable")
                .with_arg("softwareId", software_id.to_string())
        }
        BackendError::WorkflowRejected(ResolveError::FeatureUnavailable {
            software_id,
            feature,
        }) => CommandError::new("workflow.feature_unavailable")
            .with_arg("softwareId", software_id.to_string())
            .with_arg("feature", format!("{feature:?}")),
        BackendError::SoftwareOccupied {
            software_id,
            workflow_id,
        } => CommandError::new("workflow.software_occupied")
            .with_arg("softwareId", software_id.to_string())
            .with_arg("workflowId", workflow_id.to_string()),
        BackendError::Storage(_) => CommandError::new("storage.write_failed"),
        _ => CommandError::new("workflow.invalid"),
    }
}

fn capture_backend_error(error: BackendError) -> CommandError {
    match error {
        BackendError::UnknownSoftware(id) => {
            CommandError::new("capture.unknown_software").with_arg("softwareId", id.to_string())
        }
        BackendError::UnknownAdapter(id) => {
            CommandError::new("capture.unknown_adapter").with_arg("adapterId", id.to_string())
        }
        BackendError::InvalidInput("capture-adapter-empty") => {
            CommandError::new("capture.adapters_required")
        }
        BackendError::InvalidInput("capture-adapter-cannot-observe") => {
            CommandError::new("capture.adapter_cannot_observe")
        }
        _ => CommandError::new("capture.invalid_configuration"),
    }
}

fn probe_run_error(error: ProbeRunError) -> CommandError {
    let code = match error {
        ProbeRunError::InvalidInput => "capture.invalid_configuration",
        ProbeRunError::NotFound => "capture.workspace_not_found",
        ProbeRunError::AlreadyExists => "capture.workspace_exists",
        ProbeRunError::InvalidState => "capture.invalid_state",
        ProbeRunError::InvalidRun => "capture.invalid_workspace",
        ProbeRunError::Storage => "capture.write_failed",
        ProbeRunError::Observation => "capture.read_failed",
        ProbeRunError::Export => "capture.export_failed",
    };
    CommandError::new(code)
}

fn dictionary_distribution_error(error: DictionaryDistributionError) -> CommandError {
    let code = match error {
        DictionaryDistributionError::InvalidCatalog => "dictionary.catalog_invalid",
        DictionaryDistributionError::CatalogUnavailable => "dictionary.catalog_unavailable",
        DictionaryDistributionError::ReleaseMissing => "dictionary.release_missing",
        DictionaryDistributionError::ArtifactTooLarge => "dictionary.artifact_too_large",
        DictionaryDistributionError::SizeMismatch => "dictionary.artifact_size_mismatch",
        DictionaryDistributionError::DigestMismatch => "dictionary.artifact_digest_mismatch",
        DictionaryDistributionError::InvalidSignature => "dictionary.signature_invalid",
        DictionaryDistributionError::UntrustedPublisher => "dictionary.publisher_untrusted",
        DictionaryDistributionError::PublisherIdentityMismatch => {
            "dictionary.publisher_identity_mismatch"
        }
        DictionaryDistributionError::TrustUnavailable => "dictionary.trust_unavailable",
        DictionaryDistributionError::InvalidPayload => "dictionary.payload_invalid",
        DictionaryDistributionError::ReleaseIdentityMismatch => {
            "dictionary.release_identity_mismatch"
        }
        DictionaryDistributionError::LocalChangesConflict => "dictionary.local_changes_conflict",
        DictionaryDistributionError::StorageFailure => "dictionary.installation_storage_failure",
    };
    CommandError::new(code)
}

struct OfflineDictionaryCatalog;

impl DictionaryDistributionPort for OfflineDictionaryCatalog {
    fn query(&mut self, _query: &CatalogQuery) -> Result<CatalogSourcePage, CatalogPortError> {
        Err(CatalogPortError::Unavailable)
    }

    fn release(&mut self, _key: &DictionaryReleaseKey) -> Result<CatalogRelease, CatalogPortError> {
        Err(CatalogPortError::Unavailable)
    }

    fn fetch(
        &mut self,
        _download_url: &str,
        _byte_limit: u64,
    ) -> Result<Vec<u8>, CatalogPortError> {
        Err(CatalogPortError::Unavailable)
    }
}

struct OfflineArtifactTrustVerifier;

impl ArtifactTrustVerifier for OfflineArtifactTrustVerifier {
    fn verify(
        &mut self,
        _statement: &ArtifactStatement,
        _signature: &SignatureEnvelope,
    ) -> Result<PublisherIdentity, TrustVerifierError> {
        Err(TrustVerifierError::Unavailable)
    }
}

fn offline_dictionary_distribution(
    data_root: &std::path::Path,
) -> Result<DictionaryDistribution, String> {
    let store = FileDictionaryInstallStore::open(data_root)
        .map_err(|error| format!("dictionary installation startup: {error:?}"))?;
    Ok(DictionaryDistribution::new(
        Box::new(OfflineDictionaryCatalog),
        Box::new(OfflineArtifactTrustVerifier),
        Box::new(store),
        Box::new(SystemInstallationClock),
    ))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProbeRunCreateRequest {
    id: Box<str>,
    name: Box<str>,
    software_id: Box<str>,
    adapter_ids: Vec<Box<str>>,
    live_preview_enabled: bool,
    dictionary: ProbeDictionaryBindingRequest,
}

#[derive(Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
enum ProbeDictionaryBindingRequest {
    Existing {
        dictionary_id: Box<str>,
    },
    New {
        id: Box<str>,
        name: Box<str>,
        description: Box<str>,
        source_locale: Box<str>,
        target_locale: Box<str>,
    },
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProbeRunQueryRequest {
    run_id: Box<str>,
    search: Box<str>,
    #[serde(default)]
    adapter_ids: Vec<Box<str>>,
    page: usize,
    page_size: usize,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProbeTranslationEditRequest {
    run_id: Box<str>,
    source: Box<str>,
    translation: Box<str>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProbeBulkRequest {
    run_id: Box<str>,
    sources: Vec<Box<str>>,
    action: Box<str>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DictionaryCatalogQueryRequest {
    text: Box<str>,
    source_locale: Option<Box<str>>,
    target_locale: Option<Box<str>>,
    cursor: Option<Box<str>>,
    page_size: Option<u16>,
    requested_presentation_locale: Box<str>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct DictionaryCatalogReleaseView {
    catalog_id: Box<str>,
    dictionary_id: Box<str>,
    release_version: Box<str>,
    source_locale: Box<str>,
    target_locale: Box<str>,
    effective_presentation_locale: Box<str>,
    name: Box<str>,
    summary: Box<str>,
    tags: Vec<Box<str>>,
    publisher_identity: Box<str>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct DictionaryCatalogPageView {
    releases: Vec<DictionaryCatalogReleaseView>,
    next_cursor: Option<Box<str>>,
}

impl From<CatalogPage> for DictionaryCatalogPageView {
    fn from(page: CatalogPage) -> Self {
        Self {
            releases: page
                .releases()
                .iter()
                .map(|release| DictionaryCatalogReleaseView {
                    catalog_id: release.key().catalog_id().into(),
                    dictionary_id: release.key().dictionary_id().into(),
                    release_version: release.key().release_version().into(),
                    source_locale: release.source_locale().into(),
                    target_locale: release.target_locale().into(),
                    effective_presentation_locale: release.effective_presentation_locale().into(),
                    name: release.presentation().name().into(),
                    summary: release.presentation().summary().into(),
                    tags: release.presentation().tags().to_vec(),
                    publisher_identity: release.publisher_identity().as_str().into(),
                })
                .collect(),
            next_cursor: page.next_cursor().map(Into::into),
        }
    }
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum DictionaryReplacementRequest {
    RejectExisting,
    ReplaceVerified,
    ReplaceAny,
}

impl From<DictionaryReplacementRequest> for DictionaryReplacementPolicy {
    fn from(value: DictionaryReplacementRequest) -> Self {
        match value {
            DictionaryReplacementRequest::RejectExisting => Self::RejectExisting,
            DictionaryReplacementRequest::ReplaceVerified => Self::ReplaceVerified,
            DictionaryReplacementRequest::ReplaceAny => Self::ReplaceAny,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DictionaryCatalogInstallRequest {
    catalog_id: Box<str>,
    dictionary_id: Box<str>,
    release_version: Box<str>,
    replacement: DictionaryReplacementRequest,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProbeExportRequest {
    run_id: Box<str>,
    format: ProbeExportFormat,
    output_path: PathBuf,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct ProbeRunView {
    #[serde(flatten)]
    summary: ProbeRunSummary,
    dictionary_revision: u64,
    dictionary_entry_count: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DesktopStatus {
    shell_ready: bool,
    product_version: &'static str,
    api_version: u16,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct WorkflowTargetRuntimeView {
    software_id: Box<str>,
    discovered: bool,
    active: bool,
    translation_requested: bool,
    font_requested: bool,
    translation_active: bool,
    font_active: bool,
    applied_generation: Option<u64>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct WorkflowRuntimeView {
    workflow_id: Box<str>,
    targets: Vec<WorkflowTargetRuntimeView>,
    errors: BTreeMap<Box<str>, CommandError>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct WorkflowActivationView {
    workflow_id: Box<str>,
    enabled: bool,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct WorkflowRuntimeDiagnosticsView {
    workflow_id: Box<str>,
    records: Vec<WorkflowRuntimeTraceView>,
    dropped: u64,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct WorkflowRuntimeTraceView {
    software_id: Box<str>,
    software_name: Box<str>,
    adapter_name: Box<str>,
    source_text: Box<str>,
    status: Box<str>,
    text: Box<str>,
    font: Box<str>,
    generation: u64,
    publication_identity: Box<str>,
    translation_digest: Box<str>,
    font_policy_digest: Box<str>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct WorkflowCommandResult {
    definition: WorkflowView,
    activation: WorkflowActivationView,
    runtime: WorkflowRuntimeView,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct AdapterView {
    id: Box<str>,
    name: Box<str>,
    version: Box<str>,
    summary: Box<str>,
    platforms: Vec<Box<str>>,
    technologies: Vec<Box<str>>,
    features: Vec<Box<str>>,
    technical_target: Box<str>,
    configuration: Box<str>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct DesktopProductSnapshot {
    #[serde(flatten)]
    configuration: DesktopSnapshot,
    workflow_runtime_status: BTreeMap<Box<str>, WorkflowRuntimeView>,
    adapters: Vec<AdapterView>,
    font_families: Vec<Box<str>>,
}

trait WorkflowRuntimeService: Send {
    fn activate_workflow(
        &mut self,
        intent: &EffectiveWorkflowIntent,
        replace_conflicts: bool,
    ) -> WorkflowRuntimeView;

    fn stop_workflow(&mut self, intent: &EffectiveWorkflowIntent) -> WorkflowRuntimeView;

    fn refresh_workflow(&mut self, intent: &EffectiveWorkflowIntent) -> WorkflowRuntimeView;

    fn remove_software(&mut self, software_id: &str) -> Result<(), DesktopRuntimeError>;

    fn start_capture(
        &mut self,
        software_id: &str,
        spec: &glyphshift_desktop_backend::DesktopRuntimeSpec,
        configuration: CaptureConfiguration,
    ) -> Result<(), DesktopRuntimeError>;

    fn stop_capture(&mut self, software_id: &str) -> Result<(), DesktopRuntimeError>;

    fn control_capture(
        &mut self,
        software_id: &str,
        paused: bool,
    ) -> Result<(), DesktopRuntimeError>;

    fn control_runtime_diagnostics(
        &mut self,
        software_id: &str,
        enabled: bool,
    ) -> Result<(), DesktopRuntimeError>;

    fn query_runtime_diagnostics(
        &mut self,
        software_id: &str,
    ) -> Result<RuntimeTraceBatch, DesktopRuntimeError>;

    fn publish_capture(
        &mut self,
        software_id: &str,
        publication: RuntimePublication,
    ) -> Result<(), DesktopRuntimeError>;
}

impl WorkflowRuntimeService for DesktopRuntimePool {
    fn activate_workflow(
        &mut self,
        intent: &EffectiveWorkflowIntent,
        replace_conflicts: bool,
    ) -> WorkflowRuntimeView {
        let report = if replace_conflicts {
            self.replace_workflow(intent)
        } else {
            self.reconcile_workflow(intent)
        };
        workflow_runtime_view(self, intent, report, true)
    }

    fn stop_workflow(&mut self, intent: &EffectiveWorkflowIntent) -> WorkflowRuntimeView {
        let report = DesktopRuntimePool::stop_workflow(self, intent.workflow_id());
        workflow_runtime_view(self, intent, report, false)
    }

    fn refresh_workflow(&mut self, intent: &EffectiveWorkflowIntent) -> WorkflowRuntimeView {
        let report = DesktopRuntimePool::refresh_workflow(self, intent);
        workflow_runtime_view(self, intent, report, true)
    }

    fn remove_software(&mut self, software_id: &str) -> Result<(), DesktopRuntimeError> {
        self.remove(software_id)
    }

    fn start_capture(
        &mut self,
        software_id: &str,
        spec: &glyphshift_desktop_backend::DesktopRuntimeSpec,
        configuration: CaptureConfiguration,
    ) -> Result<(), DesktopRuntimeError> {
        DesktopRuntimePool::start_capture(self, software_id, spec, None, configuration).map(|_| ())
    }

    fn stop_capture(&mut self, software_id: &str) -> Result<(), DesktopRuntimeError> {
        DesktopRuntimePool::stop_capture(self, software_id).map(|_| ())
    }

    fn control_capture(
        &mut self,
        software_id: &str,
        paused: bool,
    ) -> Result<(), DesktopRuntimeError> {
        DesktopRuntimePool::control_capture(self, software_id, paused)
    }

    fn control_runtime_diagnostics(
        &mut self,
        software_id: &str,
        enabled: bool,
    ) -> Result<(), DesktopRuntimeError> {
        DesktopRuntimePool::control_runtime_diagnostics(self, software_id, enabled)
    }

    fn query_runtime_diagnostics(
        &mut self,
        software_id: &str,
    ) -> Result<RuntimeTraceBatch, DesktopRuntimeError> {
        DesktopRuntimePool::query_runtime_diagnostics(self, software_id)
    }

    fn publish_capture(
        &mut self,
        software_id: &str,
        publication: RuntimePublication,
    ) -> Result<(), DesktopRuntimeError> {
        DesktopRuntimePool::publish(self, software_id, publication)
    }
}

struct DesktopApplication {
    backend: DesktopBackend,
    dictionary_distribution: DictionaryDistribution,
    runtimes: Option<Box<dyn WorkflowRuntimeService>>,
    workflow_runtime_status: BTreeMap<Box<str>, WorkflowRuntimeView>,
    adapters: Vec<AdapterView>,
    font_families: Vec<Box<str>>,
    probe_runs: ProbeRunStore,
    active_probe_run_id: Option<Box<str>>,
}

impl DesktopApplication {
    fn open(data_root: PathBuf, runtime_root: PathBuf) -> Result<Self, String> {
        let probe_runs = ProbeRunStore::open(data_root.join("probe-runs"))
            .map_err(|error| format!("probe run startup: {error:?}"))?;
        let dictionary_distribution = offline_dictionary_distribution(&data_root)?;
        let runtime_bundle = RuntimeBundle::open(runtime_root).ok();
        let adapters = runtime_bundle
            .as_ref()
            .map(|bundle| {
                bundle
                    .translation_adapter_options()
                    .iter()
                    .map(|adapter| AdapterView {
                        id: adapter.id().into(),
                        name: adapter.name().into(),
                        version: adapter.version().into(),
                        summary: adapter.summary().into(),
                        platforms: adapter.platforms().to_vec(),
                        technologies: adapter.technologies().to_vec(),
                        features: adapter
                            .features()
                            .iter()
                            .map(|feature| adapter_feature_id(*feature).into())
                            .collect(),
                        technical_target: adapter.technical_target().into(),
                        configuration: adapter.configuration().into(),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let font_families = system_font_families();
        let environment = DesktopEnvironment::new(
            runtime_bundle
                .as_ref()
                .map(|bundle| bundle.adapter_requirements().to_vec())
                .unwrap_or_default(),
            font_families.iter().cloned(),
        );
        let backend = DesktopBackend::open_with_environment(data_root, environment)
            .map_err(|error| format!("{error:?}"))?;
        let mut application = Self {
            backend,
            dictionary_distribution,
            runtimes: runtime_bundle.map(|bundle| {
                Box::new(DesktopRuntimePool::new(bundle)) as Box<dyn WorkflowRuntimeService>
            }),
            workflow_runtime_status: BTreeMap::new(),
            adapters,
            font_families,
            probe_runs,
            active_probe_run_id: None,
        };
        application
            .restore_enabled_workflows()
            .map_err(|error| format!("{error:?}"))?;
        Ok(application)
    }

    fn snapshot(&self) -> DesktopProductSnapshot {
        let configuration = self.backend.snapshot();
        let enabled_workflows = configuration
            .activations()
            .iter()
            .map(|activation| activation.workflow_id())
            .collect::<std::collections::BTreeSet<_>>();
        let workflow_runtime_status = configuration
            .workflows()
            .iter()
            .filter_map(|workflow| {
                self.workflow_runtime_status
                    .get(workflow.id())
                    .cloned()
                    .or_else(|| {
                        self.backend
                            .effective_workflow_intent(workflow.id())
                            .ok()
                            .map(|intent| {
                                idle_workflow_runtime_view(
                                    &intent,
                                    enabled_workflows.contains(workflow.id()),
                                )
                            })
                    })
                    .map(|runtime| (Box::<str>::from(workflow.id()), runtime))
            })
            .collect();
        DesktopProductSnapshot {
            configuration,
            workflow_runtime_status,
            adapters: self.adapters.clone(),
            font_families: self.font_families.clone(),
        }
    }

    fn probe_run_view(&self, summary: ProbeRunSummary) -> Result<ProbeRunView, CommandError> {
        let dictionary = self
            .backend
            .dictionary(summary.dictionary_id())
            .map_err(|_| {
                CommandError::new("dictionary.not_found")
                    .with_arg("dictionaryId", summary.dictionary_id())
            })?;
        Ok(ProbeRunView {
            summary,
            dictionary_revision: dictionary.revision(),
            dictionary_entry_count: dictionary.entries().len(),
        })
    }

    fn probe_dictionary_snapshot(
        &self,
        dictionary_id: &str,
    ) -> Result<ProbeDictionarySnapshot, CommandError> {
        let dictionary = self.backend.dictionary(dictionary_id).map_err(|_| {
            CommandError::new("dictionary.not_found").with_arg("dictionaryId", dictionary_id)
        })?;
        ProbeDictionarySnapshot::new(
            dictionary.revision(),
            dictionary
                .entries()
                .iter()
                .map(|entry| ProbeDictionaryEntry::new(entry.source(), entry.translation())),
        )
        .map_err(probe_run_error)
    }

    fn probe_run_list(&mut self) -> Result<Vec<ProbeRunView>, CommandError> {
        let summaries = self.probe_runs.list().map_err(probe_run_error)?;
        summaries
            .into_iter()
            .map(|summary| self.probe_run_view(summary))
            .collect()
    }

    fn create_probe_run(
        &mut self,
        request: ProbeRunCreateRequest,
    ) -> Result<ProbeRunView, CommandError> {
        if self.active_probe_run_id.is_some() {
            return Err(CommandError::new("capture.already_active"));
        }
        if request.live_preview_enabled && !self.adapters_support_preview(&request.adapter_ids) {
            return Err(CommandError::new("capture.preview_unavailable"));
        }
        let dictionary_id = match request.dictionary {
            ProbeDictionaryBindingRequest::Existing { dictionary_id } => {
                self.backend.dictionary(&dictionary_id).map_err(|_| {
                    CommandError::new("dictionary.not_found")
                        .with_arg("dictionaryId", dictionary_id.to_string())
                })?;
                dictionary_id
            }
            ProbeDictionaryBindingRequest::New {
                id,
                name,
                description,
                source_locale,
                target_locale,
            } => {
                let create = DictionaryCreate::new(id.clone(), name, source_locale, target_locale)
                    .with_description(description);
                self.backend
                    .create_dictionary(create)
                    .map_err(|_| CommandError::new("dictionary.invalid_create"))?;
                id
            }
        };
        let create = ProbeRunCreate::new(
            request.id,
            request.name,
            request.software_id,
            dictionary_id,
            request.adapter_ids,
            request.live_preview_enabled,
        )
        .map_err(probe_run_error)?;
        let summary = self.probe_runs.create(create).map_err(probe_run_error)?;
        self.start_probe_run_runtime(summary.id())
    }

    fn delete_probe_runs(&mut self, run_ids: &[Box<str>]) -> Result<(), CommandError> {
        if run_ids.is_empty() {
            return Err(CommandError::new("capture.invalid_configuration"));
        }
        for run_id in run_ids {
            if self.active_probe_run_id.as_deref() == Some(run_id) {
                return Err(CommandError::new("capture.invalid_state"));
            }
            self.probe_runs.delete(run_id).map_err(probe_run_error)?;
        }
        Ok(())
    }

    fn resume_probe_run(&mut self, run_id: &str) -> Result<ProbeRunView, CommandError> {
        self.start_probe_run_runtime(run_id)
    }

    fn start_probe_run_runtime(&mut self, run_id: &str) -> Result<ProbeRunView, CommandError> {
        if self.active_probe_run_id.is_some() {
            return Err(CommandError::new("capture.already_active"));
        }
        let summary = self.probe_runs.summary(run_id).map_err(probe_run_error)?;
        if matches!(
            summary.status(),
            ProbeRunStatus::Running | ProbeRunStatus::Paused
        ) {
            return Err(CommandError::new("capture.already_active"));
        }
        let spec = self
            .backend
            .capture_runtime_spec(summary.software_id(), summary.adapter_ids())
            .map_err(capture_backend_error)?;
        let configuration = self
            .probe_runs
            .capture_configuration(run_id, DEFAULT_MAX_ENTRIES)
            .map_err(probe_run_error)?;
        self.runtimes
            .as_mut()
            .ok_or_else(|| CommandError::new("runtime.unavailable"))?
            .start_capture(summary.software_id(), &spec, configuration)
            .map_err(|error| runtime_command_error(error, true))?;
        self.active_probe_run_id = Some(run_id.into());
        self.probe_runs
            .set_status(run_id, ProbeRunStatus::Running)
            .map_err(probe_run_error)?;
        if let Err(error) = self.publish_probe_preview_if_active(run_id) {
            if let Some(runtimes) = self.runtimes.as_mut() {
                let _ = runtimes.stop_capture(summary.software_id());
            }
            let _ = self
                .probe_runs
                .set_status(run_id, ProbeRunStatus::Interrupted);
            self.active_probe_run_id = None;
            return Err(error);
        }
        let summary = self.probe_runs.summary(run_id).map_err(probe_run_error)?;
        self.probe_run_view(summary)
    }

    fn set_probe_run_paused(
        &mut self,
        run_id: &str,
        paused: bool,
    ) -> Result<ProbeRunView, CommandError> {
        if self.active_probe_run_id.as_deref() != Some(run_id) {
            return Err(CommandError::new("capture.not_active"));
        }
        let summary = self.probe_runs.summary(run_id).map_err(probe_run_error)?;
        self.runtimes
            .as_mut()
            .ok_or_else(|| CommandError::new("runtime.unavailable"))?
            .control_capture(summary.software_id(), paused)
            .map_err(|error| runtime_command_error(error, false))?;
        let summary = self
            .probe_runs
            .set_status(
                run_id,
                if paused {
                    ProbeRunStatus::Paused
                } else {
                    ProbeRunStatus::Running
                },
            )
            .map_err(probe_run_error)?;
        self.probe_run_view(summary)
    }

    fn disconnect_probe_run(&mut self, run_id: &str) -> Result<ProbeRunView, CommandError> {
        if self.active_probe_run_id.as_deref() != Some(run_id) {
            return Err(CommandError::new("capture.not_active"));
        }
        let summary = self.probe_runs.summary(run_id).map_err(probe_run_error)?;
        self.runtimes
            .as_mut()
            .ok_or_else(|| CommandError::new("runtime.unavailable"))?
            .stop_capture(summary.software_id())
            .map_err(|error| runtime_command_error(error, false))?;
        self.active_probe_run_id = None;
        let summary = self
            .probe_runs
            .set_status(run_id, ProbeRunStatus::Ready)
            .map_err(probe_run_error)?;
        self.probe_run_view(summary)
    }

    fn probe_run_summary(&mut self, run_id: &str) -> Result<ProbeRunView, CommandError> {
        let summary = self.probe_runs.summary(run_id).map_err(probe_run_error)?;
        self.probe_run_view(summary)
    }

    fn probe_run_entries(
        &mut self,
        request: ProbeRunQueryRequest,
    ) -> Result<ProbeEntryPage, CommandError> {
        let query = ProbeQuery::new(request.search, request.page, request.page_size)
            .and_then(|query| query.with_adapter_ids(request.adapter_ids))
            .map_err(probe_run_error)?;
        let summary = self
            .probe_runs
            .summary(&request.run_id)
            .map_err(probe_run_error)?;
        let dictionary = self.probe_dictionary_snapshot(summary.dictionary_id())?;
        self.probe_runs
            .query_entries(&request.run_id, &query, &dictionary)
            .map_err(probe_run_error)
    }

    fn edit_probe_translation(
        &mut self,
        request: ProbeTranslationEditRequest,
    ) -> Result<ProbeRunView, CommandError> {
        let summary = self
            .probe_runs
            .summary(&request.run_id)
            .map_err(probe_run_error)?;
        let dictionary = self
            .backend
            .dictionary(summary.dictionary_id())
            .cloned()
            .map_err(|_| CommandError::new("dictionary.not_found"))?;
        let existing = dictionary
            .entries()
            .iter()
            .find(|entry| entry.source() == request.source.as_ref());
        let translation = request.translation.trim();
        let changed = if translation.is_empty() {
            if existing.is_some() {
                self.backend
                    .delete_dictionary_entries(
                        dictionary.id(),
                        [request.source.clone()],
                        dictionary.revision(),
                    )
                    .map_err(|_| CommandError::new("dictionary.invalid_update"))?;
                true
            } else {
                false
            }
        } else if existing.is_some_and(|entry| entry.translation() == translation) {
            false
        } else {
            self.backend
                .upsert_dictionary_entry(
                    dictionary.id(),
                    DictionaryEntryCreate::new(request.source.clone(), translation),
                    existing.map(|entry| entry.source()),
                    dictionary.revision(),
                )
                .map_err(|_| CommandError::new("dictionary.invalid_update"))?;
            true
        };
        if changed {
            self.reconcile_enabled_workflows()?;
        }
        self.publish_probe_preview_if_active(&request.run_id)?;
        self.probe_run_summary(&request.run_id)
    }

    fn bulk_probe_entries(
        &mut self,
        request: ProbeBulkRequest,
    ) -> Result<ProbeRunView, CommandError> {
        match request.action.as_ref() {
            "ignore" => {
                self.probe_runs
                    .set_ignored(&request.run_id, &request.sources, true)
                    .map_err(probe_run_error)?;
            }
            "restore" => {
                self.probe_runs
                    .set_ignored(&request.run_id, &request.sources, false)
                    .map_err(probe_run_error)?;
            }
            "clear_translations" => {
                let summary = self
                    .probe_runs
                    .summary(&request.run_id)
                    .map_err(probe_run_error)?;
                let dictionary = self
                    .backend
                    .dictionary(summary.dictionary_id())
                    .cloned()
                    .map_err(|_| CommandError::new("dictionary.not_found"))?;
                let existing = dictionary
                    .entries()
                    .iter()
                    .map(|entry| entry.source())
                    .collect::<BTreeSet<_>>();
                let sources = request
                    .sources
                    .iter()
                    .filter(|source| existing.contains(source.as_ref()))
                    .cloned()
                    .collect::<Vec<_>>();
                if !sources.is_empty() {
                    self.backend
                        .delete_dictionary_entries(dictionary.id(), sources, dictionary.revision())
                        .map_err(|_| CommandError::new("dictionary.invalid_update"))?;
                    self.reconcile_enabled_workflows()?;
                }
            }
            _ => return Err(CommandError::new("capture.invalid_configuration")),
        }
        self.publish_probe_preview_if_active(&request.run_id)?;
        self.probe_run_summary(&request.run_id)
    }

    fn export_probe_run(&mut self, request: ProbeExportRequest) -> Result<(), CommandError> {
        if !request.output_path.is_absolute() {
            return Err(CommandError::new("capture.export_failed"));
        }
        let summary = self
            .probe_runs
            .summary(&request.run_id)
            .map_err(probe_run_error)?;
        let content = if request.format == ProbeExportFormat::DictionaryJson {
            self.backend
                .dictionary_json(summary.dictionary_id())
                .map_err(|_| CommandError::new("capture.export_failed"))?
        } else {
            let dictionary = self.probe_dictionary_snapshot(summary.dictionary_id())?;
            self.probe_runs
                .export(&request.run_id, request.format, &dictionary)
                .map_err(probe_run_error)?
        };
        std::fs::write(request.output_path, content)
            .map_err(|_| CommandError::new("capture.export_failed"))
    }

    fn adapters_support_preview(&self, adapter_ids: &[Box<str>]) -> bool {
        !adapter_ids.is_empty()
            && adapter_ids.iter().all(|adapter_id| {
                self.adapters.iter().any(|adapter| {
                    adapter.id == *adapter_id
                        && adapter
                            .features
                            .iter()
                            .any(|feature| feature.as_ref() == "textReplace")
                })
            })
    }

    fn publish_probe_preview_if_active(&mut self, run_id: &str) -> Result<(), CommandError> {
        if self.active_probe_run_id.as_deref() != Some(run_id) {
            return Ok(());
        }
        let summary = self.probe_runs.summary(run_id).map_err(probe_run_error)?;
        if !summary.live_preview_enabled() {
            return Ok(());
        }
        let spec = self
            .backend
            .capture_runtime_spec(summary.software_id(), summary.adapter_ids())
            .map_err(capture_backend_error)?;
        let generation = summary.preview_generation().saturating_add(1);
        let mut locations = BTreeSet::<Box<str>>::new();
        for operator in spec.publication().route().operators() {
            match operator {
                RouteOperator::Direct { location }
                | RouteOperator::ContextualHeading { location, .. } => {
                    locations.insert(location.clone());
                }
                RouteOperator::Fallback {
                    locations: fallback,
                } => {
                    locations.extend(fallback.iter().cloned());
                }
                RouteOperator::Unknown { .. }
                | RouteOperator::NativeCode
                | RouteOperator::Script
                | RouteOperator::Io => {}
            }
        }
        let dictionary = self.probe_dictionary_snapshot(summary.dictionary_id())?;
        let entries = self
            .probe_runs
            .preview_entries(run_id, &dictionary)
            .map_err(probe_run_error)?;
        let mut snapshot = TranslationSnapshot::empty(Generation::new(generation));
        for location in locations {
            for entry in &entries {
                snapshot = snapshot.with_entry_for_adapters(
                    location.clone(),
                    entry.source(),
                    entry.translation(),
                    summary.adapter_ids().iter().cloned(),
                );
            }
        }
        let publication = RuntimePublication::new(
            spec.publication().route().clone(),
            snapshot,
            FontPolicy::empty(),
        );
        self.runtimes
            .as_mut()
            .ok_or_else(|| CommandError::new("runtime.unavailable"))?
            .publish_capture(summary.software_id(), publication)
            .map_err(|error| runtime_command_error(error, false))?;
        self.probe_runs
            .set_preview_generation(run_id, generation)
            .map_err(probe_run_error)?;
        Ok(())
    }

    fn dictionary_detail(&self, dictionary_id: &str) -> Result<DictionaryView, CommandError> {
        self.backend
            .dictionary(dictionary_id)
            .cloned()
            .map_err(|_| {
                CommandError::new("dictionary.not_found").with_arg("dictionaryId", dictionary_id)
            })
    }

    fn query_dictionary_catalog(
        &mut self,
        request: DictionaryCatalogQueryRequest,
    ) -> Result<DictionaryCatalogPageView, CommandError> {
        let mut query = CatalogQuery::new(request.text);
        if let Some(source_locale) = request.source_locale {
            query = query.with_source_locale(source_locale);
        }
        if let Some(target_locale) = request.target_locale {
            query = query.with_target_locale(target_locale);
        }
        if let Some(cursor) = request.cursor {
            query = query.with_cursor(cursor);
        }
        if let Some(page_size) = request.page_size {
            query = query.with_page_size(page_size);
        }
        self.dictionary_distribution
            .query(&query, &request.requested_presentation_locale)
            .map(DictionaryCatalogPageView::from)
            .map_err(dictionary_distribution_error)
    }

    fn install_dictionary_release(
        &mut self,
        request: DictionaryCatalogInstallRequest,
    ) -> Result<DesktopProductSnapshot, CommandError> {
        let release = DictionaryReleaseKey::new(
            request.catalog_id,
            request.dictionary_id,
            request.release_version,
        )
        .map_err(|_| CommandError::new("dictionary.catalog_invalid"))?;
        self.dictionary_distribution
            .install(&InstallRequest::new(release, request.replacement.into()))
            .map_err(dictionary_distribution_error)?;
        self.backend.reload_dictionaries().map_err(|_| {
            dictionary_distribution_error(DictionaryDistributionError::StorageFailure)
        })?;
        self.reconcile_enabled_workflows()?;
        Ok(self.snapshot())
    }

    fn create_dictionary(
        &mut self,
        create: DictionaryCreate,
    ) -> Result<DesktopProductSnapshot, CommandError> {
        self.backend
            .create_dictionary(create)
            .map_err(|_| CommandError::new("dictionary.invalid_create"))?;
        Ok(self.snapshot())
    }

    fn update_dictionary(
        &mut self,
        edit: DictionaryEdit,
    ) -> Result<DesktopProductSnapshot, CommandError> {
        self.backend
            .update_dictionary(edit)
            .map_err(|_| CommandError::new("dictionary.invalid_update"))?;
        self.reconcile_enabled_workflows()?;
        Ok(self.snapshot())
    }

    fn delete_dictionaries(
        &mut self,
        dictionary_ids: &[Box<str>],
    ) -> Result<DesktopProductSnapshot, CommandError> {
        let referenced_by_probe = self
            .probe_runs
            .list()
            .map_err(probe_run_error)?
            .into_iter()
            .any(|run| {
                dictionary_ids
                    .iter()
                    .any(|id| id.as_ref() == run.dictionary_id())
            });
        if referenced_by_probe {
            return Err(CommandError::new("dictionary.referenced"));
        }
        self.backend
            .delete_dictionaries(dictionary_ids.iter().map(AsRef::as_ref))
            .map_err(|_| CommandError::new("dictionary.referenced"))?;
        Ok(self.snapshot())
    }

    fn workflow_detail(&self, workflow_id: &str) -> Result<WorkflowView, CommandError> {
        self.backend.workflow(workflow_id).map_err(|_| {
            CommandError::new("workflow.not_found").with_arg("workflowId", workflow_id)
        })
    }

    fn create_workflow(
        &mut self,
        create: WorkflowCreate,
    ) -> Result<DesktopProductSnapshot, CommandError> {
        self.backend
            .create_workflow(create)
            .map_err(|_| CommandError::new("workflow.invalid_create"))?;
        Ok(self.snapshot())
    }

    fn update_workflow(
        &mut self,
        edit: WorkflowEdit,
    ) -> Result<DesktopProductSnapshot, CommandError> {
        let workflow_id = self
            .backend
            .update_workflow(edit)
            .map_err(|_| CommandError::new("workflow.invalid_update"))?
            .id()
            .to_owned();
        self.reconcile_workflow_if_enabled(&workflow_id)?;
        Ok(self.snapshot())
    }

    fn copy_workflow(
        &mut self,
        source_workflow_id: &str,
        new_workflow_id: Box<str>,
        name: Box<str>,
    ) -> Result<DesktopProductSnapshot, CommandError> {
        self.backend
            .copy_workflow(source_workflow_id, new_workflow_id, name)
            .map_err(|_| CommandError::new("workflow.invalid_copy"))?;
        Ok(self.snapshot())
    }

    fn delete_workflows(
        &mut self,
        workflow_ids: &[Box<str>],
    ) -> Result<DesktopProductSnapshot, CommandError> {
        self.backend
            .delete_workflows(workflow_ids.iter().map(AsRef::as_ref))
            .map_err(|_| CommandError::new("workflow.enabled_delete"))?;
        for workflow_id in workflow_ids {
            self.workflow_runtime_status.remove(workflow_id.as_ref());
        }
        Ok(self.snapshot())
    }

    fn reconcile_workflow_if_enabled(&mut self, workflow_id: &str) -> Result<(), CommandError> {
        if !self
            .backend
            .enabled_workflow_ids()
            .iter()
            .any(|enabled| enabled.as_ref() == workflow_id)
        {
            return Ok(());
        }
        let intent = self
            .backend
            .effective_workflow_intent(workflow_id)
            .map_err(|_| CommandError::new("workflow.reconcile_invalid"))?;
        let runtime = self.runtimes.as_mut().map_or_else(
            || unavailable_workflow_runtime_view(&intent, true),
            |runtimes| runtimes.activate_workflow(&intent, false),
        );
        self.workflow_runtime_status
            .insert(workflow_id.into(), runtime);
        Ok(())
    }

    fn reconcile_enabled_workflows(&mut self) -> Result<(), CommandError> {
        let workflow_ids = self
            .backend
            .enabled_workflow_ids()
            .iter()
            .map(|workflow_id| workflow_id.to_string())
            .collect::<Vec<_>>();
        for workflow_id in workflow_ids {
            self.reconcile_workflow_if_enabled(&workflow_id)?;
        }
        Ok(())
    }

    fn restore_enabled_workflows(&mut self) -> Result<(), CommandError> {
        self.reconcile_enabled_workflows()
    }

    fn refresh_workflows(&mut self) -> Result<DesktopProductSnapshot, CommandError> {
        let workflow_ids = self
            .backend
            .enabled_workflow_ids()
            .iter()
            .map(|workflow_id| workflow_id.to_string())
            .collect::<Vec<_>>();
        for workflow_id in workflow_ids {
            let intent = self
                .backend
                .effective_workflow_intent(&workflow_id)
                .map_err(|_| CommandError::new("workflow.refresh_invalid"))?;
            let runtime = self.runtimes.as_mut().map_or_else(
                || unavailable_workflow_runtime_view(&intent, true),
                |runtimes| runtimes.refresh_workflow(&intent),
            );
            self.workflow_runtime_status
                .insert(workflow_id.into(), runtime);
        }
        Ok(self.snapshot())
    }

    fn control_workflow_diagnostics(
        &mut self,
        workflow_id: &str,
        enabled: bool,
    ) -> Result<(), CommandError> {
        let intent = self
            .backend
            .effective_workflow_intent(workflow_id)
            .map_err(workflow_activation_command_error)?;
        let runtimes = self.runtimes.as_mut().ok_or_else(runtime_unavailable)?;
        let mut controlled = Vec::new();
        for target in intent.targets() {
            if let Err(error) = runtimes.control_runtime_diagnostics(target.software_id(), enabled)
            {
                if enabled {
                    for software_id in controlled {
                        let _ = runtimes.control_runtime_diagnostics(software_id, false);
                    }
                }
                return Err(runtime_diagnostics_error(error, target.software_id()));
            }
            controlled.push(target.software_id());
        }
        Ok(())
    }

    fn workflow_diagnostics(
        &mut self,
        workflow_id: &str,
    ) -> Result<WorkflowRuntimeDiagnosticsView, CommandError> {
        let intent = self
            .backend
            .effective_workflow_intent(workflow_id)
            .map_err(workflow_activation_command_error)?;
        let snapshot = self.backend.snapshot();
        let software_names = snapshot
            .software()
            .iter()
            .map(|software| (software.id(), software.name()))
            .collect::<BTreeMap<_, _>>();
        let adapter_names = self
            .adapters
            .iter()
            .map(|adapter| (adapter.id.as_ref(), adapter.name.as_ref()))
            .collect::<BTreeMap<_, _>>();
        let runtimes = self.runtimes.as_mut().ok_or_else(runtime_unavailable)?;
        let mut records = Vec::new();
        let mut dropped = 0_u64;
        for target in intent.targets() {
            let batch = runtimes
                .query_runtime_diagnostics(target.software_id())
                .map_err(|error| runtime_diagnostics_error(error, target.software_id()))?;
            dropped = dropped.saturating_add(batch.dropped());
            records.extend(batch.records().iter().map(|record| {
                workflow_runtime_trace_view(
                    target.software_id(),
                    software_names
                        .get(target.software_id())
                        .copied()
                        .unwrap_or("未知软件"),
                    adapter_names
                        .get(record.adapter_id())
                        .copied()
                        .unwrap_or("未知适配器"),
                    record,
                )
            }));
        }
        Ok(WorkflowRuntimeDiagnosticsView {
            workflow_id: workflow_id.into(),
            records,
            dropped,
        })
    }

    fn enable_workflow(
        &mut self,
        workflow_id: &str,
        replace_conflicts: bool,
    ) -> Result<WorkflowCommandResult, CommandError> {
        let intent = self
            .backend
            .effective_workflow_intent(workflow_id)
            .map_err(workflow_activation_command_error)?;
        if replace_conflicts {
            self.backend
                .replace_workflow_activation(workflow_id)
                .map_err(workflow_activation_command_error)?;
        } else {
            self.backend
                .enable_workflow(workflow_id)
                .map_err(workflow_activation_command_error)?;
        }
        let runtime = self.runtimes.as_mut().map_or_else(
            || unavailable_workflow_runtime_view(&intent, true),
            |runtimes| runtimes.activate_workflow(&intent, replace_conflicts),
        );
        let definition = self
            .backend
            .workflow(workflow_id)
            .map_err(|_| CommandError::new("workflow.invalid"))?;
        self.workflow_runtime_status.retain(|candidate, _| {
            self.backend
                .enabled_workflow_ids()
                .iter()
                .any(|enabled| enabled == candidate)
        });
        self.workflow_runtime_status
            .insert(workflow_id.into(), runtime.clone());
        Ok(WorkflowCommandResult {
            definition,
            activation: WorkflowActivationView {
                workflow_id: workflow_id.into(),
                enabled: true,
            },
            runtime,
        })
    }

    fn disable_workflow(
        &mut self,
        workflow_id: &str,
    ) -> Result<WorkflowCommandResult, CommandError> {
        let intent = self
            .backend
            .effective_workflow_intent(workflow_id)
            .map_err(|_| CommandError::new("workflow.disable_invalid"))?;
        self.backend
            .disable_workflow(workflow_id)
            .map_err(|_| CommandError::new("workflow.disable_failed"))?;
        let runtime = self.runtimes.as_mut().map_or_else(
            || unavailable_workflow_runtime_view(&intent, false),
            |runtimes| runtimes.stop_workflow(&intent),
        );
        let definition = self
            .backend
            .workflow(workflow_id)
            .map_err(|_| CommandError::new("workflow.invalid"))?;
        self.workflow_runtime_status
            .insert(workflow_id.into(), runtime.clone());
        Ok(WorkflowCommandResult {
            definition,
            activation: WorkflowActivationView {
                workflow_id: workflow_id.into(),
                enabled: false,
            },
            runtime,
        })
    }

    fn add_software(
        &mut self,
        executable_path: String,
    ) -> Result<DesktopProductSnapshot, CommandError> {
        self.backend
            .add_software(ExecutableSelection::new(executable_path))
            .map_err(|_| CommandError::new("software.invalid_executable"))?;
        Ok(self.snapshot())
    }

    fn select_software(
        &mut self,
        extension_id: &str,
    ) -> Result<DesktopProductSnapshot, CommandError> {
        self.backend
            .select_software(extension_id)
            .map_err(|_| CommandError::new("software.select_failed"))?;
        Ok(self.snapshot())
    }

    fn remove_software(
        &mut self,
        extension_id: &str,
    ) -> Result<DesktopProductSnapshot, CommandError> {
        if let Some(runtimes) = self.runtimes.as_mut() {
            runtimes
                .remove_software(extension_id)
                .map_err(|_| CommandError::new("software.runtime_stop_unconfirmed"))?;
        }
        self.backend
            .remove_software(extension_id)
            .map_err(|_| CommandError::new("software.delete_failed"))?;
        Ok(self.snapshot())
    }

    fn update_software(
        &mut self,
        extension_id: String,
        display_name: String,
        description: String,
        executable_path: String,
    ) -> Result<DesktopProductSnapshot, CommandError> {
        let edit = SoftwareEdit::new(extension_id.clone(), display_name, executable_path)
            .with_description(description);
        self.backend
            .validate_software_edit(&edit)
            .map_err(|_| CommandError::new("software.invalid_update"))?;
        if let Some(runtimes) = self.runtimes.as_mut() {
            runtimes
                .remove_software(&extension_id)
                .map_err(|_| CommandError::new("software.runtime_stop_unconfirmed"))?;
        }
        self.backend
            .update_software(edit)
            .map_err(|_| CommandError::new("software.invalid_update"))?;
        self.reconcile_enabled_workflows()?;
        Ok(self.snapshot())
    }
}

fn adapter_feature_id(feature: Feature) -> &'static str {
    match feature {
        Feature::TextObserve => "textObserve",
        Feature::TextReplace => "textReplace",
        Feature::FontSubstitute => "fontSubstitute",
        Feature::LayoutAdjust => "layoutAdjust",
        Feature::ResourceReplace => "resourceReplace",
    }
}

fn runtime_command_error(error: DesktopRuntimeError, enabling: bool) -> CommandError {
    match error {
        DesktopRuntimeError::UnknownTarget => CommandError::new("runtime.target_not_found"),
        DesktopRuntimeError::SessionRejected if enabling => {
            CommandError::new("runtime.session_rejected")
        }
        DesktopRuntimeError::BundleUnavailable => CommandError::new("runtime.bundle_unavailable"),
        DesktopRuntimeError::ProtocolRejected if enabling => {
            CommandError::new("runtime.component_incompatible")
        }
        DesktopRuntimeError::ActivationRejected(reason) if enabling => match reason {
            HostOperationFailure::TargetProcessUnavailable
            | HostOperationFailure::RemoteMemoryUnavailable
            | HostOperationFailure::RemoteThreadUnavailable => {
                CommandError::new("runtime.target_access_failed")
            }
            HostOperationFailure::RuntimeModuleUnavailable => {
                CommandError::new("runtime.component_load_failed")
            }
            HostOperationFailure::RuntimeExportUnavailable
            | HostOperationFailure::TargetRuntimeRejected(_) => {
                CommandError::new("runtime.component_incompatible")
            }
            HostOperationFailure::RemoteThreadTimeout => {
                CommandError::new("runtime.activation_timed_out")
            }
            HostOperationFailure::ControllerRejected => {
                CommandError::new("runtime.activation_failed")
            }
        },
        _ if enabling => CommandError::new("runtime.activation_failed"),
        _ => CommandError::new("runtime.stop_unconfirmed"),
    }
}

fn runtime_diagnostics_error(error: DesktopRuntimeError, software_id: &str) -> CommandError {
    let code = match error {
        DesktopRuntimeError::InvalidState => "runtime.diagnostics_inactive",
        DesktopRuntimeError::ControllerUnavailable
        | DesktopRuntimeError::BundleUnavailable
        | DesktopRuntimeError::ProtocolRejected => "runtime.diagnostics_unavailable",
        _ => "runtime.diagnostics_failed",
    };
    CommandError::new(code).with_arg("softwareId", software_id)
}

fn workflow_runtime_trace_view(
    software_id: &str,
    software_name: &str,
    adapter_name: &str,
    record: &RuntimeTraceRecord,
) -> WorkflowRuntimeTraceView {
    use glyphshift_desktop_runtime::{RuntimeFontOutcome, RuntimeTextOutcome, RuntimeTraceStatus};

    WorkflowRuntimeTraceView {
        software_id: software_id.into(),
        software_name: software_name.into(),
        adapter_name: adapter_name.into(),
        source_text: record.source_text().into(),
        status: match record.status() {
            RuntimeTraceStatus::NoMatch => "no_match",
            RuntimeTraceStatus::Matched => "matched",
            RuntimeTraceStatus::ContextRecorded => "context_recorded",
            RuntimeTraceStatus::InvalidObservation => "invalid_observation",
            RuntimeTraceStatus::InvalidRouteProgram => "invalid_route_program",
            RuntimeTraceStatus::ExecutionLimitExceeded => "execution_limit_exceeded",
            RuntimeTraceStatus::StateLimitExceeded => "state_limit_exceeded",
        }
        .into(),
        text: match record.text() {
            RuntimeTextOutcome::Unmatched => "unmatched",
            RuntimeTextOutcome::Replaced => "replaced",
        }
        .into(),
        font: match record.font() {
            RuntimeFontOutcome::Unmatched => "unmatched",
            RuntimeFontOutcome::Protected => "protected",
            RuntimeFontOutcome::Substituted => "substituted",
        }
        .into(),
        generation: record.generation(),
        publication_identity: digest_hex(record.publication_identity()).into(),
        translation_digest: digest_hex(record.translation_digest()).into(),
        font_policy_digest: digest_hex(record.font_policy_digest()).into(),
    }
}

fn digest_hex(digest: [u8; 32]) -> String {
    use std::fmt::Write;

    digest
        .iter()
        .fold(String::with_capacity(64), |mut output, byte| {
            let _ = write!(output, "{byte:02x}");
            output
        })
}

fn workflow_runtime_view(
    runtimes: &DesktopRuntimePool,
    intent: &EffectiveWorkflowIntent,
    report: Result<WorkflowReconcileReport, DesktopRuntimeError>,
    enabling: bool,
) -> WorkflowRuntimeView {
    let (reported_statuses, reported_errors, operation_error) = match report {
        Ok(report) => (
            report
                .statuses()
                .iter()
                .cloned()
                .map(|status| (Box::<str>::from(status.application_id()), status))
                .collect::<BTreeMap<_, _>>(),
            report.errors().clone(),
            None,
        ),
        Err(error) => (BTreeMap::new(), BTreeMap::new(), Some(error)),
    };
    let mut errors = BTreeMap::new();
    let targets = intent
        .targets()
        .iter()
        .map(|target| {
            let software_id = target.software_id();
            let status = runtimes
                .status(software_id)
                .or_else(|| reported_statuses.get(software_id).cloned());
            if let Some(error) = reported_errors
                .get(software_id)
                .copied()
                .or(operation_error)
            {
                errors.insert(software_id.into(), runtime_command_error(error, enabling));
            }
            target_runtime_view(
                software_id,
                status.as_ref(),
                enabling.then_some(target.requested_features()),
            )
        })
        .collect();
    WorkflowRuntimeView {
        workflow_id: intent.workflow_id().into(),
        targets,
        errors,
    }
}

fn unavailable_workflow_runtime_view(
    intent: &EffectiveWorkflowIntent,
    enabling: bool,
) -> WorkflowRuntimeView {
    WorkflowRuntimeView {
        workflow_id: intent.workflow_id().into(),
        targets: intent
            .targets()
            .iter()
            .map(|target| {
                target_runtime_view(
                    target.software_id(),
                    None,
                    enabling.then_some(target.requested_features()),
                )
            })
            .collect(),
        errors: intent
            .targets()
            .iter()
            .map(|target| {
                (
                    Box::<str>::from(target.software_id()),
                    runtime_command_error(DesktopRuntimeError::BundleUnavailable, enabling),
                )
            })
            .collect(),
    }
}

fn idle_workflow_runtime_view(
    intent: &EffectiveWorkflowIntent,
    enabled: bool,
) -> WorkflowRuntimeView {
    WorkflowRuntimeView {
        workflow_id: intent.workflow_id().into(),
        targets: intent
            .targets()
            .iter()
            .map(|target| {
                target_runtime_view(
                    target.software_id(),
                    None,
                    enabled.then_some(target.requested_features()),
                )
            })
            .collect(),
        errors: BTreeMap::new(),
    }
}

fn target_runtime_view(
    software_id: &str,
    runtime: Option<&DesktopRuntimeStatus>,
    requested_fallback: Option<&[Feature]>,
) -> WorkflowTargetRuntimeView {
    let requested = |feature| {
        runtime.is_some_and(|runtime| runtime.is_feature_requested(feature))
            || runtime.is_none()
                && requested_fallback.is_some_and(|features| features.contains(&feature))
    };
    WorkflowTargetRuntimeView {
        software_id: software_id.into(),
        discovered: runtime.is_some_and(|runtime| runtime.targets().next().is_some()),
        active: runtime.is_some_and(DesktopRuntimeStatus::is_active),
        translation_requested: requested(Feature::TextReplace),
        font_requested: requested(Feature::FontSubstitute),
        translation_active: runtime
            .is_some_and(|runtime| runtime.is_feature_active(Feature::TextReplace)),
        font_active: runtime
            .is_some_and(|runtime| runtime.is_feature_active(Feature::FontSubstitute)),
        applied_generation: runtime
            .and_then(DesktopRuntimeStatus::applied_generation)
            .map(glyphshift_domain::Generation::value),
    }
}

fn normalize_font_registry_label(label: &str) -> Option<&str> {
    let label = label.trim().trim_start_matches('@').trim();
    let family = label
        .rfind(" (")
        .filter(|_| label.ends_with(')'))
        .map_or(label, |suffix| &label[..suffix]);
    (!family.is_empty()).then_some(family)
}

#[cfg(target_os = "windows")]
fn system_font_families() -> Vec<Box<str>> {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ};
    use winreg::RegKey;

    let mut families = BTreeSet::new();
    for hive in [HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER] {
        let Ok(fonts) =
            RegKey::predef(hive).open_subkey_with_flags(WINDOWS_FONT_REGISTRY_KEY, KEY_READ)
        else {
            continue;
        };
        for (label, _) in fonts.enum_values().flatten() {
            if let Some(family) = normalize_font_registry_label(&label) {
                families.insert(Box::<str>::from(family));
            }
        }
    }
    families.into_iter().collect()
}

#[cfg(not(target_os = "windows"))]
fn system_font_families() -> Vec<Box<str>> {
    Vec::new()
}

fn settings_command_error(error: SettingsError) -> CommandError {
    match error {
        SettingsError::InvalidData => CommandError::new("settings.invalid_data"),
        SettingsError::Storage => CommandError::new("settings.write_failed"),
    }
}

fn workspace_unavailable() -> CommandError {
    CommandError::new("workspace.unavailable")
}

fn runtime_unavailable() -> CommandError {
    CommandError::new("runtime.unavailable")
}

#[tauri::command]
fn desktop_settings(
    settings: State<'_, Mutex<AppSettingsStore>>,
) -> Result<AppSettings, CommandError> {
    settings
        .lock()
        .map_err(|_| CommandError::new("settings.unavailable"))
        .and_then(|settings| settings.current().map_err(settings_command_error))
}

#[tauri::command]
fn desktop_update_settings(
    update: AppSettingsUpdate,
    settings: State<'_, Mutex<AppSettingsStore>>,
) -> Result<AppSettings, CommandError> {
    settings
        .lock()
        .map_err(|_| CommandError::new("settings.unavailable"))?
        .update(update)
        .map_err(settings_command_error)
}

#[tauri::command]
fn desktop_status() -> DesktopStatus {
    DesktopStatus {
        shell_ready: true,
        product_version: env!("CARGO_PKG_VERSION"),
        api_version: DESKTOP_API_VERSION,
    }
}

#[tauri::command]
fn desktop_snapshot(
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<DesktopProductSnapshot, CommandError> {
    application
        .lock()
        .map_err(|_| workspace_unavailable())
        .map(|application| application.snapshot())
}

#[tauri::command]
fn desktop_probe_runs(
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<Vec<ProbeRunView>, CommandError> {
    application
        .lock()
        .map_err(|_| workspace_unavailable())?
        .probe_run_list()
}

#[tauri::command]
fn desktop_create_probe_run(
    request: ProbeRunCreateRequest,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<ProbeRunView, CommandError> {
    application
        .lock()
        .map_err(|_| runtime_unavailable())?
        .create_probe_run(request)
}

#[tauri::command]
fn desktop_delete_probe_runs(
    run_ids: Vec<String>,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<(), CommandError> {
    let run_ids = run_ids
        .into_iter()
        .map(Box::<str>::from)
        .collect::<Vec<_>>();
    application
        .lock()
        .map_err(|_| workspace_unavailable())?
        .delete_probe_runs(&run_ids)
}

#[tauri::command]
fn desktop_resume_probe_run(
    run_id: String,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<ProbeRunView, CommandError> {
    application
        .lock()
        .map_err(|_| runtime_unavailable())?
        .resume_probe_run(&run_id)
}

#[tauri::command]
fn desktop_set_probe_run_paused(
    run_id: String,
    paused: bool,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<ProbeRunView, CommandError> {
    application
        .lock()
        .map_err(|_| runtime_unavailable())?
        .set_probe_run_paused(&run_id, paused)
}

#[tauri::command]
fn desktop_disconnect_probe_run(
    run_id: String,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<ProbeRunView, CommandError> {
    application
        .lock()
        .map_err(|_| runtime_unavailable())?
        .disconnect_probe_run(&run_id)
}

#[tauri::command]
fn desktop_probe_run_summary(
    run_id: String,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<ProbeRunView, CommandError> {
    application
        .lock()
        .map_err(|_| workspace_unavailable())?
        .probe_run_summary(&run_id)
}

#[tauri::command]
fn desktop_probe_run_entries(
    request: ProbeRunQueryRequest,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<ProbeEntryPage, CommandError> {
    application
        .lock()
        .map_err(|_| workspace_unavailable())?
        .probe_run_entries(request)
}

#[tauri::command]
fn desktop_edit_probe_translation(
    request: ProbeTranslationEditRequest,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<ProbeRunView, CommandError> {
    application
        .lock()
        .map_err(|_| runtime_unavailable())?
        .edit_probe_translation(request)
}

#[tauri::command]
fn desktop_bulk_probe_entries(
    request: ProbeBulkRequest,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<ProbeRunView, CommandError> {
    application
        .lock()
        .map_err(|_| runtime_unavailable())?
        .bulk_probe_entries(request)
}

#[tauri::command]
fn desktop_export_probe_run(
    request: ProbeExportRequest,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<(), CommandError> {
    application
        .lock()
        .map_err(|_| workspace_unavailable())?
        .export_probe_run(request)
}

#[tauri::command]
fn desktop_dictionary(
    dictionary_id: String,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<DictionaryView, CommandError> {
    application
        .lock()
        .map_err(|_| workspace_unavailable())?
        .dictionary_detail(&dictionary_id)
}

#[tauri::command]
fn desktop_query_dictionary_catalog(
    request: DictionaryCatalogQueryRequest,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<DictionaryCatalogPageView, CommandError> {
    application
        .lock()
        .map_err(|_| workspace_unavailable())?
        .query_dictionary_catalog(request)
}

#[tauri::command]
fn desktop_install_dictionary_release(
    request: DictionaryCatalogInstallRequest,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<DesktopProductSnapshot, CommandError> {
    application
        .lock()
        .map_err(|_| workspace_unavailable())?
        .install_dictionary_release(request)
}

#[tauri::command]
fn desktop_create_dictionary(
    create: DictionaryCreate,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<DesktopProductSnapshot, CommandError> {
    application
        .lock()
        .map_err(|_| workspace_unavailable())?
        .create_dictionary(create)
}

#[tauri::command]
fn desktop_update_dictionary(
    edit: DictionaryEdit,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<DesktopProductSnapshot, CommandError> {
    application
        .lock()
        .map_err(|_| workspace_unavailable())?
        .update_dictionary(edit)
}

#[tauri::command]
fn desktop_delete_dictionaries(
    dictionary_ids: Vec<String>,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<DesktopProductSnapshot, CommandError> {
    let dictionary_ids = dictionary_ids
        .into_iter()
        .map(Box::<str>::from)
        .collect::<Vec<_>>();
    application
        .lock()
        .map_err(|_| workspace_unavailable())?
        .delete_dictionaries(&dictionary_ids)
}

#[tauri::command]
fn desktop_workflow(
    workflow_id: String,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<WorkflowView, CommandError> {
    application
        .lock()
        .map_err(|_| workspace_unavailable())?
        .workflow_detail(&workflow_id)
}

#[tauri::command]
fn desktop_create_workflow(
    create: WorkflowCreate,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<DesktopProductSnapshot, CommandError> {
    application
        .lock()
        .map_err(|_| workspace_unavailable())?
        .create_workflow(create)
}

#[tauri::command]
fn desktop_update_workflow(
    edit: WorkflowEdit,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<DesktopProductSnapshot, CommandError> {
    application
        .lock()
        .map_err(|_| workspace_unavailable())?
        .update_workflow(edit)
}

#[tauri::command]
fn desktop_copy_workflow(
    source_workflow_id: String,
    new_workflow_id: String,
    name: String,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<DesktopProductSnapshot, CommandError> {
    application
        .lock()
        .map_err(|_| workspace_unavailable())?
        .copy_workflow(&source_workflow_id, new_workflow_id.into(), name.into())
}

#[tauri::command]
fn desktop_delete_workflows(
    workflow_ids: Vec<String>,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<DesktopProductSnapshot, CommandError> {
    let workflow_ids = workflow_ids
        .into_iter()
        .map(Box::<str>::from)
        .collect::<Vec<_>>();
    application
        .lock()
        .map_err(|_| workspace_unavailable())?
        .delete_workflows(&workflow_ids)
}

#[tauri::command]
fn desktop_add_software(
    executable_path: String,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<DesktopProductSnapshot, CommandError> {
    application
        .lock()
        .map_err(|_| workspace_unavailable())?
        .add_software(executable_path)
}

#[tauri::command]
fn desktop_select_software(
    extension_id: String,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<DesktopProductSnapshot, CommandError> {
    application
        .lock()
        .map_err(|_| workspace_unavailable())?
        .select_software(&extension_id)
}

#[tauri::command]
fn desktop_update_software(
    extension_id: String,
    display_name: String,
    description: String,
    executable_path: String,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<DesktopProductSnapshot, CommandError> {
    application
        .lock()
        .map_err(|_| workspace_unavailable())?
        .update_software(extension_id, display_name, description, executable_path)
}

#[tauri::command]
fn desktop_enable_workflow(
    workflow_id: String,
    replace_conflicts: bool,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<WorkflowCommandResult, CommandError> {
    application
        .lock()
        .map_err(|_| runtime_unavailable())?
        .enable_workflow(&workflow_id, replace_conflicts)
}

#[tauri::command]
fn desktop_disable_workflow(
    workflow_id: String,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<WorkflowCommandResult, CommandError> {
    application
        .lock()
        .map_err(|_| runtime_unavailable())?
        .disable_workflow(&workflow_id)
}

#[tauri::command]
fn desktop_refresh_workflows(
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<DesktopProductSnapshot, CommandError> {
    application
        .lock()
        .map_err(|_| runtime_unavailable())?
        .refresh_workflows()
}

#[tauri::command]
fn desktop_control_workflow_diagnostics(
    workflow_id: String,
    enabled: bool,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<(), CommandError> {
    application
        .lock()
        .map_err(|_| runtime_unavailable())?
        .control_workflow_diagnostics(&workflow_id, enabled)
}

#[tauri::command]
fn desktop_workflow_diagnostics(
    workflow_id: String,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<WorkflowRuntimeDiagnosticsView, CommandError> {
    application
        .lock()
        .map_err(|_| runtime_unavailable())?
        .workflow_diagnostics(&workflow_id)
}

#[tauri::command]
fn desktop_remove_software(
    extension_id: String,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<DesktopProductSnapshot, CommandError> {
    application
        .lock()
        .map_err(|_| workspace_unavailable())?
        .remove_software(&extension_id)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_root = std::env::var_os("GLYPHSHIFT_DATA_ROOT")
                .map(PathBuf::from)
                .map_or_else(
                    || {
                        app.path()
                            .app_data_dir()
                            .map(|path| path.join("workspace"))
                            .map_err(|error| std::io::Error::other(error.to_string()))
                    },
                    Ok,
                )?;
            let runtime_root = std::env::var_os("GLYPHSHIFT_RUNTIME_ROOT")
                .map(PathBuf::from)
                .unwrap_or(app.path().resource_dir()?.join("runtime"));
            let settings = AppSettingsStore::open(&data_root)
                .map_err(|error| std::io::Error::other(format!("settings startup: {error:?}")))?;
            let application =
                DesktopApplication::open(data_root, runtime_root).map_err(std::io::Error::other)?;
            app.manage(Mutex::new(settings));
            app.manage(Mutex::new(application));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            desktop_status,
            desktop_settings,
            desktop_update_settings,
            desktop_snapshot,
            desktop_probe_runs,
            desktop_create_probe_run,
            desktop_delete_probe_runs,
            desktop_resume_probe_run,
            desktop_set_probe_run_paused,
            desktop_disconnect_probe_run,
            desktop_probe_run_summary,
            desktop_probe_run_entries,
            desktop_edit_probe_translation,
            desktop_bulk_probe_entries,
            desktop_export_probe_run,
            desktop_dictionary,
            desktop_query_dictionary_catalog,
            desktop_install_dictionary_release,
            desktop_create_dictionary,
            desktop_update_dictionary,
            desktop_delete_dictionaries,
            desktop_workflow,
            desktop_create_workflow,
            desktop_update_workflow,
            desktop_copy_workflow,
            desktop_delete_workflows,
            desktop_add_software,
            desktop_update_software,
            desktop_select_software,
            desktop_enable_workflow,
            desktop_disable_workflow,
            desktop_refresh_workflows,
            desktop_control_workflow_diagnostics,
            desktop_workflow_diagnostics,
            desktop_remove_software
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Glyphshift desktop shell");
}

#[cfg(test)]
mod tests {
    use super::*;
    use glyphshift_adapter_registry::{
        AdapterRequirement, AdapterVersion, AdapterVersionRequirement,
    };
    use glyphshift_desktop_backend::{
        DictionaryCreate, DictionaryEdit, DictionaryEntryCreate, WorkflowCreate, WorkflowEdit,
        WorkflowTargetCreate,
    };
    use glyphshift_dictionary_distribution::{
        ArtifactPresentation, DictionaryArtifactDescriptor, FixedInstallationClock,
        InMemoryDictionaryCatalog, InMemoryTrustVerifier, Sha256Digest,
    };
    use sha2::{Digest, Sha256};
    use std::fs;
    use std::sync::{Arc, Mutex as StdMutex};
    use tempfile::tempdir;

    const TEST_ADAPTER_ID: &str = "test.inline";
    const TEST_DICTIONARY_URL: &str = "https://catalog.example/dictionary.json";

    fn fixture_dictionary_distribution(data_root: &std::path::Path) -> DictionaryDistribution {
        let payload = glyphshift_dictionary_package::DictionaryPackage::create(
            glyphshift_dictionary_package::DictionaryCreate::new(
                "dictionary.catalog",
                "Catalog Dictionary",
                "en-US",
                "zh-CN",
            )
            .with_release_version("1.2.0")
            .with_entries([glyphshift_dictionary_package::DictionaryEntryCreate::new(
                "Open", "打开",
            )]),
        )
        .expect("dictionary package")
        .encode_json()
        .expect("encode dictionary")
        .into_bytes();
        let digest: [u8; 32] = Sha256::digest(&payload).into();
        let publisher = PublisherIdentity::new("publisher.example").expect("publisher");
        let signature =
            SignatureEnvelope::new("fixture", "test-key", "signed-statement").expect("signature");
        let release = CatalogRelease::new(
            DictionaryReleaseKey::new("glyphshift.official", "dictionary.catalog", "1.2.0")
                .expect("release key"),
            "en-US",
            "zh-CN",
            "en-US",
            vec![
                ArtifactPresentation::new("en-US", "Catalog Dictionary", "Menu translations")
                    .expect("English presentation"),
                ArtifactPresentation::new("zh-CN", "目录词典", "菜单翻译")
                    .expect("Chinese presentation"),
            ],
            DictionaryArtifactDescriptor::new(
                payload.len() as u64,
                Sha256Digest::new(digest),
                [TEST_DICTIONARY_URL],
                publisher.clone(),
                signature.clone(),
            )
            .expect("artifact descriptor"),
        )
        .expect("catalog release");
        DictionaryDistribution::new(
            Box::new(
                InMemoryDictionaryCatalog::new()
                    .with_release(release.clone())
                    .with_artifact(TEST_DICTIONARY_URL, payload),
            ),
            Box::new(InMemoryTrustVerifier::new().with_trusted_artifact(
                ArtifactStatement::for_release(&release),
                signature,
                publisher,
            )),
            Box::new(
                FileDictionaryInstallStore::open(data_root).expect("dictionary install store"),
            ),
            Box::new(FixedInstallationClock::new(1_700_000_000_000)),
        )
    }

    #[test]
    fn dictionary_distribution_errors_keep_stable_product_codes() {
        let cases = [
            (
                DictionaryDistributionError::CatalogUnavailable,
                "dictionary.catalog_unavailable",
            ),
            (
                DictionaryDistributionError::ReleaseMissing,
                "dictionary.release_missing",
            ),
            (
                DictionaryDistributionError::DigestMismatch,
                "dictionary.artifact_digest_mismatch",
            ),
            (
                DictionaryDistributionError::InvalidSignature,
                "dictionary.signature_invalid",
            ),
            (
                DictionaryDistributionError::UntrustedPublisher,
                "dictionary.publisher_untrusted",
            ),
            (
                DictionaryDistributionError::InvalidPayload,
                "dictionary.payload_invalid",
            ),
            (
                DictionaryDistributionError::LocalChangesConflict,
                "dictionary.local_changes_conflict",
            ),
            (
                DictionaryDistributionError::StorageFailure,
                "dictionary.installation_storage_failure",
            ),
        ];
        for (error, expected_code) in cases {
            let value = serde_json::to_value(dictionary_distribution_error(error))
                .expect("serialize command error");
            assert_eq!(value["schemaVersion"], 1);
            assert_eq!(value["code"], expected_code);
        }
    }

    #[test]
    fn unconfigured_catalog_reports_offline_without_affecting_the_local_library() {
        let (mut application, _calls, _software_id, _data_root) = workflow_application();
        let before = application.snapshot().configuration.dictionaries().to_vec();

        let error = application
            .query_dictionary_catalog(DictionaryCatalogQueryRequest {
                text: "menu".into(),
                source_locale: Some("en-US".into()),
                target_locale: Some("zh-CN".into()),
                cursor: None,
                page_size: Some(20),
                requested_presentation_locale: "zh-CN".into(),
            })
            .expect_err("catalog remains explicitly offline without configuration");
        let value = serde_json::to_value(error).expect("serialize command error");

        assert_eq!(value["code"], "dictionary.catalog_unavailable");
        assert_eq!(application.snapshot().configuration.dictionaries(), before);
    }

    #[test]
    fn catalog_query_and_install_return_presentation_and_refreshed_installation_summary() {
        let (mut application, _calls, _software_id, data_root) = workflow_application();
        application.dictionary_distribution = fixture_dictionary_distribution(data_root.path());

        let page = application
            .query_dictionary_catalog(DictionaryCatalogQueryRequest {
                text: "Catalog".into(),
                source_locale: Some("en-US".into()),
                target_locale: Some("zh-CN".into()),
                cursor: None,
                page_size: Some(20),
                requested_presentation_locale: "zh-CN".into(),
            })
            .expect("query configured catalog");
        assert_eq!(page.releases.len(), 1);
        assert_eq!(page.releases[0].name.as_ref(), "目录词典");
        assert_eq!(
            page.releases[0].effective_presentation_locale.as_ref(),
            "zh-CN"
        );

        let snapshot = application
            .install_dictionary_release(DictionaryCatalogInstallRequest {
                catalog_id: "glyphshift.official".into(),
                dictionary_id: "dictionary.catalog".into(),
                release_version: "1.2.0".into(),
                replacement: DictionaryReplacementRequest::RejectExisting,
            })
            .expect("install catalog release");
        let dictionary = snapshot
            .configuration
            .dictionaries()
            .iter()
            .find(|dictionary| dictionary.id() == "dictionary.catalog")
            .expect("installed dictionary summary");
        assert_eq!(dictionary.installation().state(), "verified");
        assert_eq!(
            dictionary.installation().verified_publisher(),
            Some("publisher.example")
        );
    }

    #[derive(Default)]
    struct WorkflowRuntimeCalls {
        enabled: Vec<(Box<str>, bool, Vec<Box<str>>)>,
        disabled: Vec<Box<str>>,
        refreshed: Vec<Box<str>>,
        captures_started: Vec<Box<str>>,
        captures_stopped: Vec<Box<str>>,
    }

    struct RecordingWorkflowRuntime {
        calls: Arc<StdMutex<WorkflowRuntimeCalls>>,
    }

    impl WorkflowRuntimeService for RecordingWorkflowRuntime {
        fn activate_workflow(
            &mut self,
            intent: &glyphshift_desktop_backend::EffectiveWorkflowIntent,
            replace_conflicts: bool,
        ) -> WorkflowRuntimeView {
            let software_ids = intent
                .targets()
                .iter()
                .map(|target| Box::<str>::from(target.software_id()))
                .collect::<Vec<_>>();
            self.calls.lock().expect("runtime call log").enabled.push((
                intent.workflow_id().into(),
                replace_conflicts,
                software_ids.clone(),
            ));
            WorkflowRuntimeView {
                workflow_id: intent.workflow_id().into(),
                targets: intent
                    .targets()
                    .iter()
                    .map(|target| WorkflowTargetRuntimeView {
                        software_id: target.software_id().into(),
                        discovered: true,
                        active: true,
                        translation_requested: true,
                        font_requested: false,
                        translation_active: true,
                        font_active: false,
                        applied_generation: Some(
                            target.runtime_spec().publication().generation().value(),
                        ),
                    })
                    .collect(),
                errors: BTreeMap::new(),
            }
        }

        fn stop_workflow(
            &mut self,
            intent: &glyphshift_desktop_backend::EffectiveWorkflowIntent,
        ) -> WorkflowRuntimeView {
            self.calls
                .lock()
                .expect("runtime call log")
                .disabled
                .push(intent.workflow_id().into());
            WorkflowRuntimeView {
                workflow_id: intent.workflow_id().into(),
                targets: intent
                    .targets()
                    .iter()
                    .map(|target| WorkflowTargetRuntimeView {
                        software_id: target.software_id().into(),
                        discovered: true,
                        active: false,
                        translation_requested: false,
                        font_requested: false,
                        translation_active: false,
                        font_active: false,
                        applied_generation: None,
                    })
                    .collect(),
                errors: BTreeMap::new(),
            }
        }

        fn refresh_workflow(
            &mut self,
            intent: &glyphshift_desktop_backend::EffectiveWorkflowIntent,
        ) -> WorkflowRuntimeView {
            self.calls
                .lock()
                .expect("runtime call log")
                .refreshed
                .push(intent.workflow_id().into());
            self.activate_workflow(intent, false)
        }

        fn remove_software(&mut self, _software_id: &str) -> Result<(), DesktopRuntimeError> {
            Ok(())
        }

        fn start_capture(
            &mut self,
            software_id: &str,
            _spec: &glyphshift_desktop_backend::DesktopRuntimeSpec,
            configuration: CaptureConfiguration,
        ) -> Result<(), DesktopRuntimeError> {
            self.calls
                .lock()
                .expect("runtime call log")
                .captures_started
                .push(software_id.into());
            glyphshift_capture::FileCaptureSink::start(configuration)
                .and_then(glyphshift_capture::FileCaptureSink::finish)
                .map(|_| ())
                .map_err(|_| DesktopRuntimeError::SessionRejected)
        }

        fn stop_capture(&mut self, software_id: &str) -> Result<(), DesktopRuntimeError> {
            self.calls
                .lock()
                .expect("runtime call log")
                .captures_stopped
                .push(software_id.into());
            Ok(())
        }

        fn control_capture(
            &mut self,
            _software_id: &str,
            _paused: bool,
        ) -> Result<(), DesktopRuntimeError> {
            Ok(())
        }

        fn control_runtime_diagnostics(
            &mut self,
            _software_id: &str,
            _enabled: bool,
        ) -> Result<(), DesktopRuntimeError> {
            Ok(())
        }

        fn query_runtime_diagnostics(
            &mut self,
            _software_id: &str,
        ) -> Result<RuntimeTraceBatch, DesktopRuntimeError> {
            use glyphshift_desktop_runtime::{
                RuntimeFontOutcome, RuntimeTextOutcome, RuntimeTraceStatus,
            };

            Ok(RuntimeTraceBatch::new(
                [RuntimeTraceRecord::new(
                    TEST_ADAPTER_ID,
                    "Open",
                    RuntimeTraceStatus::Matched,
                    RuntimeTextOutcome::Replaced,
                    RuntimeFontOutcome::Protected,
                    4,
                    [0x71; 32],
                    [0x72; 32],
                    [0x73; 32],
                )],
                3,
            ))
        }

        fn publish_capture(
            &mut self,
            _software_id: &str,
            _publication: RuntimePublication,
        ) -> Result<(), DesktopRuntimeError> {
            Ok(())
        }
    }

    fn workflow_application() -> (
        DesktopApplication,
        Arc<StdMutex<WorkflowRuntimeCalls>>,
        Box<str>,
        tempfile::TempDir,
    ) {
        let data_root = tempdir().expect("temporary product data");
        let executable = data_root.path().join("SyntheticWorkflowHost.exe");
        fs::write(&executable, b"synthetic executable identity").expect("synthetic executable");
        let mut backend = DesktopBackend::open_with_environment(
            data_root.path(),
            DesktopEnvironment::new(
                [AdapterRequirement::new(
                    glyphshift_domain::AdapterId::new(TEST_ADAPTER_ID),
                    AdapterVersionRequirement::Exact(AdapterVersion::new(1, 0, 0)),
                    [
                        Feature::TextObserve,
                        Feature::TextReplace,
                        Feature::FontSubstitute,
                    ],
                )],
                Vec::<Box<str>>::new(),
            ),
        )
        .expect("open product backend");
        let software_id: Box<str> = backend
            .add_software(ExecutableSelection::new(&executable))
            .expect("add synthetic software")
            .software()[0]
            .id()
            .into();
        backend
            .create_dictionary(
                DictionaryCreate::new("dictionary.product", "产品词典", "en-US", "zh-CN")
                    .with_entries([DictionaryEntryCreate::new("Open", "打开")]),
            )
            .expect("create dictionary");
        backend
            .create_workflow(
                WorkflowCreate::new("workflow.product", "产品工作流").with_targets([
                    WorkflowTargetCreate::new(
                        software_id.clone(),
                        [TEST_ADAPTER_ID],
                        ["dictionary.product"],
                    ),
                ]),
            )
            .expect("create workflow");
        let calls = Arc::new(StdMutex::new(WorkflowRuntimeCalls::default()));
        let runtimes: Box<dyn WorkflowRuntimeService> = Box::new(RecordingWorkflowRuntime {
            calls: Arc::clone(&calls),
        });
        (
            DesktopApplication {
                backend,
                dictionary_distribution: offline_dictionary_distribution(data_root.path())
                    .expect("offline dictionary distribution"),
                runtimes: Some(runtimes),
                workflow_runtime_status: BTreeMap::new(),
                adapters: Vec::new(),
                font_families: Vec::new(),
                probe_runs: ProbeRunStore::open(data_root.path().join("probe-runs"))
                    .expect("probe run store"),
                active_probe_run_id: None,
            },
            calls,
            software_id,
            data_root,
        )
    }

    #[test]
    fn reports_the_embedded_shell_only_when_the_command_is_reachable() {
        let status = desktop_status();

        assert!(status.shell_ready);
        assert_eq!(status.product_version, env!("CARGO_PKG_VERSION"));
        assert_eq!(status.api_version, DESKTOP_API_VERSION);
    }

    #[test]
    fn font_registry_labels_become_user_facing_family_options() {
        assert_eq!(
            normalize_font_registry_label("Synthetic Sans (TrueType)"),
            Some("Synthetic Sans")
        );
        assert_eq!(
            normalize_font_registry_label("Synthetic Variable Font"),
            Some("Synthetic Variable Font")
        );
        assert_eq!(normalize_font_registry_label("  "), None);
    }

    #[test]
    fn distinguishes_a_missing_path_match_from_a_rejected_runtime() {
        let missing = serde_json::to_value(runtime_command_error(
            DesktopRuntimeError::UnknownTarget,
            true,
        ))
        .expect("serialize missing target error");
        let rejected = serde_json::to_value(runtime_command_error(
            DesktopRuntimeError::SessionRejected,
            true,
        ))
        .expect("serialize rejected session error");

        assert_eq!(missing["code"], "runtime.target_not_found");
        assert_eq!(rejected["code"], "runtime.session_rejected");
    }

    #[test]
    fn workflow_activation_error_explains_an_empty_dictionary() {
        let error = serde_json::to_value(workflow_activation_command_error(
            BackendError::WorkflowRejected(ResolveError::NoEffectiveRules {
                software_id: "software.empty".into(),
            }),
        ))
        .expect("serialize workflow activation error");

        assert_eq!(error["code"], "workflow.no_effective_rules");
        assert_eq!(error["args"]["softwareId"], "software.empty");
    }

    #[test]
    fn runtime_protocol_rejection_does_not_collapse_into_a_generic_activation_error() {
        let error = serde_json::to_value(runtime_command_error(
            DesktopRuntimeError::ProtocolRejected,
            true,
        ))
        .expect("serialize Runtime protocol rejection");

        assert_eq!(error["code"], "runtime.component_incompatible");
    }

    #[test]
    fn runtime_module_rejection_reaches_an_actionable_command_error() {
        let error = serde_json::to_value(runtime_command_error(
            DesktopRuntimeError::ActivationRejected(HostOperationFailure::RuntimeModuleUnavailable),
            true,
        ))
        .expect("serialize Runtime module rejection");

        assert_eq!(error["code"], "runtime.component_load_failed");
    }

    #[test]
    fn workflow_enable_and_disable_share_one_product_command_path() {
        let (mut application, calls, software_id, _data_root) = workflow_application();

        let enabled = application
            .enable_workflow("workflow.product", false)
            .expect("enable product workflow");

        assert_eq!(enabled.definition.id(), "workflow.product");
        assert!(enabled.activation.enabled);
        assert_eq!(enabled.runtime.workflow_id.as_ref(), "workflow.product");
        assert_eq!(enabled.runtime.targets[0].software_id, software_id);
        assert!(enabled.runtime.targets[0].active);
        assert_eq!(
            application.backend.enabled_workflow_ids(),
            &[Box::<str>::from("workflow.product")]
        );
        assert_eq!(
            calls.lock().expect("runtime call log").enabled,
            vec![(
                Box::<str>::from("workflow.product"),
                false,
                vec![software_id.clone()]
            )]
        );

        let disabled = application
            .disable_workflow("workflow.product")
            .expect("disable product workflow");

        assert!(!disabled.activation.enabled);
        assert!(!disabled.runtime.targets[0].active);
        assert!(application.backend.enabled_workflow_ids().is_empty());
        assert_eq!(
            calls.lock().expect("runtime call log").disabled,
            vec![Box::<str>::from("workflow.product")]
        );
    }

    #[test]
    fn workflow_diagnostics_exposes_public_names_and_bounded_trace_facts() {
        let (mut application, _calls, software_id, _data_root) = workflow_application();
        application.adapters.push(AdapterView {
            id: TEST_ADAPTER_ID.into(),
            name: "合成适配器".into(),
            version: "1.0.0".into(),
            summary: "合成诊断适配器".into(),
            platforms: vec!["windows".into()],
            technologies: vec!["GDI".into()],
            features: vec!["text-replace".into()],
            technical_target: "Synthetic".into(),
            configuration: "none".into(),
        });
        application
            .enable_workflow("workflow.product", false)
            .expect("enable workflow");

        application
            .control_workflow_diagnostics("workflow.product", true)
            .expect("enable workflow diagnostics");
        let diagnostics = application
            .workflow_diagnostics("workflow.product")
            .expect("query workflow diagnostics");

        assert_eq!(diagnostics.workflow_id.as_ref(), "workflow.product");
        assert_eq!(diagnostics.records.len(), 1);
        assert_eq!(
            diagnostics.records[0].software_id.as_ref(),
            software_id.as_ref()
        );
        assert_eq!(diagnostics.records[0].adapter_name.as_ref(), "合成适配器");
        assert_eq!(diagnostics.records[0].source_text.as_ref(), "Open");
        assert_eq!(diagnostics.records[0].status.as_ref(), "matched");
        assert_eq!(diagnostics.dropped, 3);
    }

    #[test]
    fn workflow_enable_keeps_desired_activation_when_runtime_is_unavailable() {
        let (mut application, _calls, software_id, _data_root) = workflow_application();
        application.runtimes = None;

        let result = application
            .enable_workflow("workflow.product", false)
            .expect("persist desired workflow activation");

        assert!(result.activation.enabled);
        assert_eq!(
            application.backend.enabled_workflow_ids(),
            &[Box::<str>::from("workflow.product")]
        );
        assert_eq!(result.runtime.targets[0].software_id, software_id);
        assert!(result.runtime.targets[0].translation_requested);
        assert!(!result.runtime.targets[0].active);
        assert!(result.runtime.errors.contains_key(software_id.as_ref()));
    }

    #[test]
    fn workflow_enable_can_explicitly_replace_a_conflicting_activation() {
        let (mut application, calls, _software_id, _data_root) = workflow_application();
        application
            .backend
            .copy_workflow("workflow.product", "workflow.replacement", "替换工作流")
            .expect("copy conflicting workflow");
        application
            .enable_workflow("workflow.product", false)
            .expect("enable original workflow");

        let replacement = application
            .enable_workflow("workflow.replacement", true)
            .expect("explicitly replace workflow");

        assert_eq!(replacement.definition.id(), "workflow.replacement");
        assert_eq!(
            application.backend.enabled_workflow_ids(),
            &[Box::<str>::from("workflow.replacement")]
        );
        assert_eq!(
            calls
                .lock()
                .expect("runtime call log")
                .enabled
                .last()
                .map(|call| (call.0.as_ref(), call.1)),
            Some(("workflow.replacement", true))
        );
    }

    #[test]
    fn workflow_runtime_serialization_does_not_expose_process_or_adapter_identity() {
        let (mut application, _calls, _software_id, _data_root) = workflow_application();
        let result = application
            .enable_workflow("workflow.product", false)
            .expect("enable workflow");

        let json = serde_json::to_value(result).expect("serialize workflow command result");
        let runtime_target = &json["runtime"]["targets"][0];

        assert_eq!(
            runtime_target
                .as_object()
                .expect("runtime target object")
                .keys()
                .cloned()
                .collect::<Vec<_>>(),
            vec![
                "active",
                "appliedGeneration",
                "discovered",
                "fontActive",
                "fontRequested",
                "softwareId",
                "translationActive",
                "translationRequested",
            ]
        );
    }

    #[test]
    fn desktop_snapshot_returns_product_configuration_activation_and_runtime_state() {
        let (mut application, _calls, software_id, _data_root) = workflow_application();
        application
            .enable_workflow("workflow.product", false)
            .expect("enable workflow");

        let json =
            serde_json::to_value(application.snapshot()).expect("serialize product snapshot");

        assert_eq!(json["workflows"][0]["id"], "workflow.product");
        assert_eq!(
            json["dictionaries"][0]["metadata"]["id"],
            "dictionary.product"
        );
        assert_eq!(
            json["dictionaries"][0]["installation"],
            serde_json::json!({
                "state": "unmanaged",
                "installedRelease": null,
                "verifiedPublisher": null,
                "updateRelease": null,
            })
        );
        assert!(json["dictionaries"][0].get("digest").is_none());
        assert!(json["dictionaries"][0].get("signature").is_none());
        assert_eq!(
            json["activations"][0],
            serde_json::json!({
                "workflowId": "workflow.product",
                "revision": 1,
            })
        );
        assert_eq!(
            json["workflowRuntimeStatus"]["workflow.product"]["targets"][0]["softwareId"],
            software_id.as_ref()
        );
        assert_eq!(
            json["workflowRuntimeStatus"]["workflow.product"]["targets"][0]["active"],
            true
        );
    }

    #[test]
    fn probe_runs_pause_release_and_reuse_one_dictionary_without_copying_entries() {
        let (mut application, _calls, software_id, _data_root) = workflow_application();
        let first = application
            .create_probe_run(ProbeRunCreateRequest {
                id: "probe-first".into(),
                name: "First probe".into(),
                software_id: software_id.clone(),
                adapter_ids: vec![TEST_ADAPTER_ID.into()],
                live_preview_enabled: false,
                dictionary: ProbeDictionaryBindingRequest::Existing {
                    dictionary_id: "dictionary.product".into(),
                },
            })
            .expect("create first probe");
        assert_eq!(first.summary.status(), ProbeRunStatus::Running);
        assert_eq!(first.summary.dictionary_id(), "dictionary.product");

        let edited = application
            .edit_probe_translation(ProbeTranslationEditRequest {
                run_id: first.summary.id().into(),
                source: "Close".into(),
                translation: "关闭".into(),
            })
            .expect("edit the bound dictionary directly");
        assert_eq!(edited.dictionary_entry_count, 2);
        assert_eq!(edited.dictionary_revision, 2);
        assert!(application
            .backend
            .dictionary("dictionary.product")
            .expect("edited shared dictionary")
            .entries()
            .iter()
            .any(|entry| entry.source() == "Close" && entry.translation() == "关闭"));

        let paused = application
            .set_probe_run_paused(first.summary.id(), true)
            .expect("pause without ending run");
        assert_eq!(paused.summary.status(), ProbeRunStatus::Paused);
        let released = application
            .disconnect_probe_run(first.summary.id())
            .expect("release runtime");
        assert_eq!(released.summary.status(), ProbeRunStatus::Ready);

        application
            .create_probe_run(ProbeRunCreateRequest {
                id: "probe-second".into(),
                name: "Second probe".into(),
                software_id,
                adapter_ids: vec![TEST_ADAPTER_ID.into()],
                live_preview_enabled: false,
                dictionary: ProbeDictionaryBindingRequest::Existing {
                    dictionary_id: "dictionary.product".into(),
                },
            })
            .expect("create second probe");
        assert_eq!(
            application.probe_run_list().expect("list probe runs").len(),
            2
        );
        assert_eq!(
            application
                .backend
                .dictionary("dictionary.product")
                .expect("shared dictionary")
                .entries()
                .len(),
            2
        );
    }

    #[test]
    fn dictionary_and_workflow_details_are_loaded_by_product_id() {
        let (application, _calls, software_id, _data_root) = workflow_application();

        let dictionary = application
            .dictionary_detail("dictionary.product")
            .expect("load dictionary detail");
        let workflow = application
            .workflow_detail("workflow.product")
            .expect("load workflow detail");

        assert_eq!(dictionary.id(), "dictionary.product");
        assert_eq!(dictionary.entries()[0].source(), "Open");
        assert_eq!(dictionary.entries()[0].translation(), "打开");
        assert_eq!(workflow.id(), "workflow.product");
        assert_eq!(workflow.targets()[0].software_id(), software_id.as_ref());
        assert_eq!(
            workflow.targets()[0].dictionary_ids(),
            &[Box::<str>::from("dictionary.product")]
        );
    }

    #[test]
    fn workflow_create_returns_the_updated_product_snapshot() {
        let (mut application, _calls, software_id, _data_root) = workflow_application();

        let snapshot = application
            .create_workflow(
                WorkflowCreate::new("workflow.secondary", "备用工作流").with_targets([
                    WorkflowTargetCreate::new(
                        software_id,
                        [TEST_ADAPTER_ID],
                        ["dictionary.product"],
                    ),
                ]),
            )
            .expect("create workflow through product command");

        assert_eq!(
            snapshot
                .configuration
                .workflows()
                .iter()
                .map(|workflow| workflow.id())
                .collect::<Vec<_>>(),
            vec!["workflow.product", "workflow.secondary"]
        );
        assert_eq!(
            application
                .workflow_detail("workflow.secondary")
                .expect("load created workflow")
                .name(),
            "备用工作流"
        );
    }

    #[test]
    fn updating_an_enabled_workflow_reconciles_its_new_generation() {
        let (mut application, _calls, software_id, _data_root) = workflow_application();
        let enabled = application
            .enable_workflow("workflow.product", false)
            .expect("enable workflow");
        let previous_generation = enabled.runtime.targets[0]
            .applied_generation
            .expect("initial applied generation");

        let snapshot = application
            .update_workflow(
                WorkflowEdit::new("workflow.product", "已更新工作流", 1).with_targets([
                    WorkflowTargetCreate::new(
                        software_id,
                        [TEST_ADAPTER_ID],
                        ["dictionary.product"],
                    ),
                ]),
            )
            .expect("update enabled workflow");

        let summary = &snapshot.configuration.workflows()[0];
        assert_eq!((summary.name(), summary.revision()), ("已更新工作流", 2));
        assert!(
            snapshot.workflow_runtime_status["workflow.product"].targets[0]
                .applied_generation
                .is_some_and(|generation| generation > previous_generation)
        );
    }

    #[test]
    fn workflow_copy_and_batch_delete_return_the_updated_product_snapshot() {
        let (mut application, _calls, _software_id, _data_root) = workflow_application();

        let copied = application
            .copy_workflow(
                "workflow.product",
                "workflow.copy".into(),
                "工作流副本".into(),
            )
            .expect("copy workflow");
        assert_eq!(
            copied
                .configuration
                .workflows()
                .iter()
                .map(|workflow| workflow.id())
                .collect::<Vec<_>>(),
            vec!["workflow.copy", "workflow.product"]
        );

        let deleted = application
            .delete_workflows(&[Box::<str>::from("workflow.copy")])
            .expect("delete copied workflow");
        assert_eq!(
            deleted
                .configuration
                .workflows()
                .iter()
                .map(|workflow| workflow.id())
                .collect::<Vec<_>>(),
            vec!["workflow.product"]
        );
    }

    #[test]
    fn dictionary_crud_reconciles_every_enabled_workflow_that_uses_the_dictionary() {
        let (mut application, _calls, _software_id, _data_root) = workflow_application();
        let created = application
            .create_dictionary(
                DictionaryCreate::new("dictionary.secondary", "备用词典", "en-US", "zh-CN")
                    .with_entries([DictionaryEntryCreate::new("Close", "关闭")]),
            )
            .expect("create dictionary");
        assert_eq!(
            created
                .configuration
                .dictionaries()
                .iter()
                .map(|dictionary| (dictionary.id(), dictionary.entry_count()))
                .collect::<Vec<_>>(),
            vec![("dictionary.product", 1), ("dictionary.secondary", 1)]
        );
        let enabled = application
            .enable_workflow("workflow.product", false)
            .expect("enable workflow");
        let previous_generation = enabled.runtime.targets[0]
            .applied_generation
            .expect("initial generation");

        let updated = application
            .update_dictionary(
                DictionaryEdit::new("dictionary.product", "产品词典 2", "en-US", "zh-CN", 1)
                    .with_entries([DictionaryEntryCreate::new("Open", "开启")]),
            )
            .expect("update active dictionary");

        assert_eq!(
            updated
                .configuration
                .dictionaries()
                .iter()
                .find(|dictionary| dictionary.id() == "dictionary.product")
                .map(|dictionary| (dictionary.name(), dictionary.revision())),
            Some(("产品词典 2", 2))
        );
        assert!(
            updated.workflow_runtime_status["workflow.product"].targets[0]
                .applied_generation
                .is_some_and(|generation| generation > previous_generation)
        );

        let deleted = application
            .delete_dictionaries(&[Box::<str>::from("dictionary.secondary")])
            .expect("delete unreferenced dictionary");
        assert_eq!(
            deleted
                .configuration
                .dictionaries()
                .iter()
                .map(|dictionary| dictionary.id())
                .collect::<Vec<_>>(),
            vec!["dictionary.product"]
        );
    }

    #[test]
    fn persisted_activations_are_restored_and_refreshed_as_workflow_runtime_state() {
        let (mut application, _calls, _software_id, data_root) = workflow_application();
        application
            .enable_workflow("workflow.product", false)
            .expect("persist enabled workflow");
        drop(application);
        let backend = DesktopBackend::open_with_environment(
            data_root.path(),
            DesktopEnvironment::new(
                [AdapterRequirement::new(
                    glyphshift_domain::AdapterId::new(TEST_ADAPTER_ID),
                    AdapterVersionRequirement::Exact(AdapterVersion::new(1, 0, 0)),
                    [
                        Feature::TextObserve,
                        Feature::TextReplace,
                        Feature::FontSubstitute,
                    ],
                )],
                Vec::<Box<str>>::new(),
            ),
        )
        .expect("reopen product backend");
        let calls = Arc::new(StdMutex::new(WorkflowRuntimeCalls::default()));
        let runtimes: Box<dyn WorkflowRuntimeService> = Box::new(RecordingWorkflowRuntime {
            calls: Arc::clone(&calls),
        });
        let mut reopened = DesktopApplication {
            backend,
            dictionary_distribution: offline_dictionary_distribution(data_root.path())
                .expect("offline dictionary distribution"),
            runtimes: Some(runtimes),
            workflow_runtime_status: BTreeMap::new(),
            adapters: Vec::new(),
            font_families: Vec::new(),
            probe_runs: ProbeRunStore::open(data_root.path().join("probe-runs"))
                .expect("probe run store"),
            active_probe_run_id: None,
        };

        reopened
            .restore_enabled_workflows()
            .expect("restore enabled workflow runtime");
        let restored = reopened.snapshot();
        assert!(restored.workflow_runtime_status["workflow.product"].targets[0].active);

        let refreshed = reopened
            .refresh_workflows()
            .expect("refresh enabled workflows");
        assert!(refreshed.workflow_runtime_status["workflow.product"].targets[0].active);
        assert_eq!(
            calls.lock().expect("runtime call log").refreshed,
            vec![Box::<str>::from("workflow.product")]
        );
    }

    #[test]
    fn software_mutations_return_the_same_product_snapshot_shape() {
        let (mut application, _calls, software_id, _data_root) = workflow_application();
        let executable_path = application.backend.snapshot().software()[0]
            .executable_path()
            .expect("bound executable path")
            .to_owned();

        let snapshot = application
            .update_software(
                software_id.to_string(),
                "合成软件 2".to_owned(),
                "默认合成场景".to_owned(),
                executable_path,
            )
            .expect("update software through product command");

        assert_eq!(snapshot.configuration.software()[0].name(), "合成软件 2");
        assert_eq!(
            snapshot.configuration.software()[0].description(),
            "默认合成场景"
        );
        assert!(snapshot
            .workflow_runtime_status
            .contains_key("workflow.product"));
    }
}

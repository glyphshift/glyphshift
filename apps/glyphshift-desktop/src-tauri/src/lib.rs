mod entry_resolution;
mod exit;
mod updates;
mod data;
mod ai;
mod ai_models;
mod window_controls;
mod command_error;
mod dictionary;
mod font_catalog;
mod probe;
mod probe_transfer;
mod quick_probe;
mod settings;
mod shortcut;
mod software;
mod workflow;
mod workflow_collection;
mod workflow_lifecycle;
mod workflow_shortcut;

use command_error::CommandError;
use dictionary::offline_dictionary_distribution;
#[cfg(test)]
use dictionary::{
    dictionary_distribution_error, dictionary_export_error, dictionary_import_error,
    DictionaryCatalogInstallRequest, DictionaryCatalogQueryRequest, DictionaryReplacementRequest,
};
use glyphshift_capture::{
    CaptureConfiguration, ProbeDictionaryEntry, ProbeDictionarySnapshot, ProbeEntryPage,
    ProbeExportFormat, ProbeQuery, ProbeRunCreate, ProbeRunError, ProbeRunStatus, ProbeRunStore,
    ProbeRunSummary, ProbeRunUpdate, ProbeTranslationFilter, DEFAULT_MAX_ENTRIES,
};
use glyphshift_controller_windows::{
    current_process_is_elevated, foreground_windows_executable, inspect_windows_executable,
    launch_process_elevated, running_windows_executables, WindowsElevationError, WindowsExecutable,
};
use glyphshift_desktop_backend::{
    BackendError, DesktopBackend, DesktopEnvironment, DesktopSnapshot, DictionaryCreate,
    DictionaryEdit, DictionaryEntryCreate, DictionaryView, EffectiveWorkflowIntent,
    ExecutableSelection, SoftwareEdit, WorkflowCreate, WorkflowEdit, WorkflowView,
};
use glyphshift_desktop_runtime::{
    DesktopRuntimeError, DesktopRuntimePool, DesktopRuntimeStatus, HostOperationFailure,
    RuntimeBundle, RuntimeTraceBatch, RuntimeTraceRecord, TargetExecutionOwner,
    WorkflowReconcileReport,
};
use glyphshift_dictionary_distribution::{
    ArtifactStatement, ArtifactTrustVerifier, CatalogPage, CatalogPortError, CatalogQuery,
    CatalogRelease, CatalogSourcePage, DictionaryDistribution, DictionaryDistributionError,
    DictionaryDistributionPort, DictionaryReleaseKey, DictionaryReplacementPolicy,
    FileDictionaryInstallStore, InstallRequest, PublisherIdentity, SignatureEnvelope,
    SystemInstallationClock, TrustVerifierError,
};
use glyphshift_domain::{Feature, Generation, Placement, RouteOperator};
use glyphshift_runtime_contract::RuntimePublication;
use glyphshift_translation::{FontPolicy, TranslationSnapshot};
use glyphshift_workflow::ResolveError;
use probe::ProbeRuntimeCapability;
#[cfg(test)]
use probe::{
    capture_preview_publish_error, ProbeDictionaryBindingRequest, ProbeRunCreateRequest,
    ProbeRunUpdateRequest, ProbeTranslationEditRequest,
};
use quick_probe::QuickProbeSessionStore;
use serde::{Deserialize, Serialize};
use settings::{
    configure_launch_at_startup, AppSettings, AppSettingsStore, AppSettingsUpdate, SettingsError,
    DEFAULT_SOFTWARE_CAPTURE_SHORTCUT,
};
#[cfg(test)]
use software::{
    software_preflight_state, SoftwarePreflightState, SoftwareQuickCaptureState,
    SoftwareQuickCaptureTransition,
};
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::{Emitter, Manager, State};
use workflow::{
    idle_workflow_runtime_view, runtime_command_error, workflow_runtime_view, WorkflowRuntimeView,
};
#[cfg(test)]
use workflow::{
    runtime_command_error_with_privilege, workflow_activation_command_error,
    WorkflowTargetRuntimeView,
};

const DESKTOP_API_VERSION: u16 = 35;
const DATA_ROOT_ARGUMENT: &str = "--glyphshift-data-root";
const RUNTIME_ROOT_ARGUMENT: &str = "--glyphshift-runtime-root";

fn default_workspace_root(roaming_data_root: &Path) -> PathBuf {
    roaming_data_root.join("Glyphshift").join("workspace")
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct DesktopLaunchContext {
    data_root: Option<PathBuf>,
    runtime_root: Option<PathBuf>,
}

impl DesktopLaunchContext {
    fn current() -> Self {
        let mut context = Self::from_args(std::env::args_os().skip(1));
        if context.data_root.is_none() {
            context.data_root = std::env::var_os("GLYPHSHIFT_DATA_ROOT").map(PathBuf::from);
        }
        if context.runtime_root.is_none() {
            context.runtime_root = std::env::var_os("GLYPHSHIFT_RUNTIME_ROOT").map(PathBuf::from);
        }
        context
    }

    fn from_args(arguments: impl IntoIterator<Item = OsString>) -> Self {
        let mut context = Self::default();
        let mut arguments = arguments.into_iter();
        while let Some(argument) = arguments.next() {
            if argument == DATA_ROOT_ARGUMENT {
                context.data_root = arguments.next().map(PathBuf::from);
            } else if argument == RUNTIME_ROOT_ARGUMENT {
                context.runtime_root = arguments.next().map(PathBuf::from);
            }
        }
        context
    }

    fn elevation_arguments(&self) -> Vec<OsString> {
        let mut arguments = Vec::new();
        if let Some(data_root) = &self.data_root {
            arguments.push(OsString::from(DATA_ROOT_ARGUMENT));
            arguments.push(data_root.as_os_str().to_owned());
        }
        if let Some(runtime_root) = &self.runtime_root {
            arguments.push(OsString::from(RUNTIME_ROOT_ARGUMENT));
            arguments.push(runtime_root.as_os_str().to_owned());
        }
        arguments
    }
}

fn launch_current_process_elevated() -> Result<(), WindowsElevationError> {
    let executable =
        std::env::current_exe().map_err(|_| WindowsElevationError::InvalidExecutable)?;
    let arguments = DesktopLaunchContext::current().elevation_arguments();
    launch_process_elevated(&executable, &arguments)
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
struct AdapterView {
    id: Box<str>,
    name: Box<str>,
    version: Box<str>,
    summary: Box<str>,
    platforms: Vec<Box<str>>,
    technologies: Vec<Box<str>>,
    features: Vec<Box<str>>,
    technical_target: Box<str>,
    documentation_url: Option<Box<str>>,
    configuration: Box<str>,
    process_resident_after_deactivate: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct AdapterTargetSupport {
    placement: Placement,
    platforms: Vec<Box<str>>,
    architectures: Vec<Box<str>>,
    features: Vec<Feature>,
}

impl AdapterTargetSupport {
    fn supports(&self, platform: &str, architecture: &str, feature: Feature) -> bool {
        self.features.contains(&feature)
            && (self.platforms.is_empty()
                || self
                    .platforms
                    .iter()
                    .any(|candidate| candidate.as_ref() == platform))
            && match self.placement {
                Placement::TargetProcess => {
                    matches!(architecture, "x86" | "x86_64")
                        && (self.architectures.is_empty()
                            || self
                                .architectures
                                .iter()
                                .any(|candidate| candidate.as_ref() == architecture))
                }
                Placement::IsolatedWorker => {
                    self.architectures.is_empty()
                        || self
                            .architectures
                            .iter()
                            .any(|candidate| candidate.as_ref() == architecture)
                }
            }
    }
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
    fn control_workflow_collection(&mut self, _workflow_id: &str, _software_id: &str, _paused: bool) -> Result<(), DesktopRuntimeError> {
        Err(DesktopRuntimeError::InvalidState)
    }

    fn configure_workflow_collection(
        &mut self,
        _workflow_id: &str,
        _collections: BTreeMap<Box<str>, CaptureConfiguration>,
    ) {}

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
    ) -> Result<ProbeRuntimeCapability, DesktopRuntimeError>;

    fn stop_capture(&mut self, software_id: &str) -> Result<(), DesktopRuntimeError>;

    fn control_capture(
        &mut self,
        software_id: &str,
        paused: bool,
    ) -> Result<(), DesktopRuntimeError>;

    fn abandon_capture(&mut self, software_id: &str);

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
    fn control_workflow_collection(&mut self, workflow_id: &str, software_id: &str, paused: bool) -> Result<(), DesktopRuntimeError> {
        DesktopRuntimePool::control_workflow_collection(self, workflow_id, software_id, paused)
    }

    fn configure_workflow_collection(
        &mut self,
        workflow_id: &str,
        collections: BTreeMap<Box<str>, CaptureConfiguration>,
    ) {
        DesktopRuntimePool::configure_workflow_collection(self, workflow_id, collections);
    }

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
    ) -> Result<ProbeRuntimeCapability, DesktopRuntimeError> {
        DesktopRuntimePool::start_capture(self, software_id, spec, None, configuration)
            .map(|status| ProbeRuntimeCapability::from_status(&status))
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

    fn abandon_capture(&mut self, software_id: &str) {
        DesktopRuntimePool::abandon_capture(self, software_id);
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
    runtime_bundle_error: Option<DesktopRuntimeError>,
    workflow_runtime_status: BTreeMap<Box<str>, WorkflowRuntimeView>,
    adapters: Vec<AdapterView>,
    adapter_target_support: BTreeMap<Box<str>, AdapterTargetSupport>,
    font_families: Vec<Box<str>>,
    font_cache_root: PathBuf,
    probe_runs: ProbeRunStore,
    probe_snapshot_cache: std::cell::RefCell<Option<(Vec<(Box<str>, u64)>, std::sync::Arc<ProbeDictionarySnapshot>)>>,
    exiting: bool,
    exit_ready: bool,
    collection_filter_policy: glyphshift_ai_translation::FilterPolicy,
    collection_filter: glyphshift_ai_translation::SourceFilterCache,
    collection_versions: BTreeMap<String, (u64, Vec<(Box<str>, u64)>)>,
    pending_collection_runs: BTreeSet<String>,
    quick_probe_sessions: QuickProbeSessionStore,
    active_probe_run_id: Option<Box<str>>,
    active_probe_capability: Option<ProbeRuntimeCapability>,
    ai_locked_dictionary_id: Option<Box<str>>,
}

impl DesktopApplication {
    fn open(data_root: PathBuf, runtime_root: PathBuf, settings: &AppSettings) -> Result<Self, String> {
        let mut probe_runs = ProbeRunStore::open(data_root.join("workflow-records"))
            .map_err(|error| format!("probe run startup: {error:?}"))?;
        let quick_probe_sessions = QuickProbeSessionStore::open(&data_root)
            .map_err(|error| format!("quick probe startup: {error:?}"))?;
        let dictionary_distribution = offline_dictionary_distribution(&data_root)?;
        let runtime_bundle = RuntimeBundle::open(runtime_root);
        let runtime_bundle_error = runtime_bundle.as_ref().err().copied();
        let runtime_bundle = runtime_bundle.ok();
        if let Some(bundle) = &runtime_bundle {
            for adapter in bundle.adapter_options() {
                probe_runs.set_source_policy(adapter.id(), adapter.source_policy());
            }
        }
        let adapter_target_support = runtime_bundle
            .as_ref()
            .into_iter()
            .flat_map(|bundle| bundle.adapter_options())
            .filter(|adapter| product_adapter_enabled(adapter.features().iter().copied()))
            .map(|adapter| {
                (
                    Box::<str>::from(adapter.id()),
                    AdapterTargetSupport {
                        placement: adapter.placement(),
                        platforms: adapter.platforms().to_vec(),
                        architectures: adapter.architectures().to_vec(),
                        features: adapter.features().to_vec(),
                    },
                )
            })
            .collect();
        let adapters = runtime_bundle
            .as_ref()
            .map(|bundle| {
                bundle
                    .adapter_options()
                    .iter()
                    .filter(|adapter| product_adapter_enabled(adapter.features().iter().copied()))
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
                        documentation_url: adapter.documentation_url().map(Into::into),
                        configuration: adapter.configuration().into(),
                        process_resident_after_deactivate: adapter
                            .process_resident_after_deactivate(),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let font_families = font_catalog::load(&data_root);
        let environment = DesktopEnvironment::new(
            runtime_bundle
                .as_ref()
                .map(|bundle| {
                    bundle
                        .adapter_requirements()
                        .iter()
                        .filter(|requirement| product_adapter_enabled(requirement.features()))
                        .cloned()
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default(),
            font_families.iter().cloned(),
        );
        let mut backend = DesktopBackend::open_with_environment(&data_root, environment)
            .map_err(|error| format!("{error:?}"))?;
        let mut application = Self {
            backend,
            dictionary_distribution,
            runtimes: runtime_bundle.map(|bundle| {
                Box::new(DesktopRuntimePool::new(bundle)) as Box<dyn WorkflowRuntimeService>
            }),
            runtime_bundle_error,
            workflow_runtime_status: BTreeMap::new(),
            adapters,
            adapter_target_support,
            font_families,
            font_cache_root: data_root.clone(),
            probe_runs,
            probe_snapshot_cache: Default::default(),
            exiting: false,
            exit_ready: false,
            collection_filter_policy: settings.text_filter_policy().clone(),
            collection_filter: Default::default(),
            collection_versions: Default::default(),
            pending_collection_runs: Default::default(),
            quick_probe_sessions,
            active_probe_run_id: None,
            active_probe_capability: None,
            ai_locked_dictionary_id: None,
        };
        application
            .restore_enabled_workflows()
            .map_err(|error| format!("{error:?}"))?;
        Ok(application)
    }

    fn runtime_bundle_command_error(&self) -> Option<CommandError> {
        self.runtime_bundle_error.map(|error| match error {
            DesktopRuntimeError::AdapterAbiMismatch => {
                CommandError::new("runtime.bundle_incompatible")
            }
            _ => CommandError::new("runtime.bundle_unavailable"),
        })
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
                    .map(|runtime| (Box::<str>::from(workflow.id()), self.project_workflow_runtime(runtime)))
            })
            .collect();
        DesktopProductSnapshot {
            configuration,
            workflow_runtime_status,
            adapters: self.adapters.clone(),
            font_families: self.font_families.clone(),
        }
    }

    fn refresh_font_families(&mut self) -> Result<DesktopProductSnapshot, CommandError> {
        let font_families = font_catalog::refresh(&self.font_cache_root)
            .map_err(|_| CommandError::new("font.cache_write_failed"))?;
        self.backend
            .replace_font_families(font_families.iter().cloned());
        self.font_families = font_families;
        Ok(self.snapshot())
    }
}

fn product_adapter_enabled(features: impl IntoIterator<Item = Feature>) -> bool {
    features
        .into_iter()
        .any(|feature| feature == Feature::TextReplace)
}

fn adapter_feature_id(feature: Feature) -> &'static str {
    match feature {
        Feature::TextObserve => "textObserve",
        Feature::TextReplace => "textReplace",
        Feature::FontSubstitute => "fontSubstitute",
        Feature::FontScale => "fontScale",
        Feature::LayoutAdjust => "layoutAdjust",
        Feature::ResourceReplace => "resourceReplace",
    }
}

fn settings_command_error(error: SettingsError) -> CommandError {
    match error {
        SettingsError::InvalidData => CommandError::new("settings.invalid_data"),
        SettingsError::Storage => CommandError::new("settings.write_failed"),
        SettingsError::Startup => CommandError::new("settings.startup_failed"),
    }
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DesktopPrivilegeStatus {
    elevated: bool,
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
    app: tauri::AppHandle,
    mut update: AppSettingsUpdate,
    application: State<'_, Mutex<DesktopApplication>>,
    settings: State<'_, Mutex<AppSettingsStore>>,
) -> Result<AppSettings, CommandError> {
    let software_capture_shortcut =
        shortcut::normalize_global_shortcut(update.software_capture_shortcut())?;
    update.set_software_capture_shortcut(software_capture_shortcut.clone());
    let mut settings = settings
        .lock()
        .map_err(|_| CommandError::new("settings.unavailable"))?;
    if settings.software_capture_shortcut() != software_capture_shortcut.as_ref() {
        return Err(CommandError::new("settings.shortcut_update_failed"));
    }
    let previous_topmost = settings.current().map_err(settings_command_error)?.always_on_top();
    let next_topmost = update.always_on_top();
    let previous_launch_at_startup = settings.launch_at_startup();
    let launch_at_startup_changed = previous_launch_at_startup != update.launch_at_startup();
    if launch_at_startup_changed {
        configure_launch_at_startup(update.launch_at_startup()).map_err(settings_command_error)?;
    }
    let mut application = application.lock().map_err(|_| workspace_unavailable())?;
    if previous_topmost != next_topmost {
        if window_controls::set_topmost(&app, next_topmost).is_err() {
            if launch_at_startup_changed { let _ = configure_launch_at_startup(previous_launch_at_startup); }
            return Err(CommandError::new("settings.window_failed"));
        }
    }
    match settings.update(update) {
        Ok(saved) => {
            application.set_collection_filter_policy(saved.text_filter_policy().clone());
            window_controls::update_labels(&app, &saved);
            // The workflow loop republishes changed decision inputs on its next reconciliation.
            Ok(saved)
        },
        Err(error) => {
            if previous_topmost != next_topmost { let _ = window_controls::set_topmost(&app, previous_topmost); }
            if launch_at_startup_changed {
                let _ = configure_launch_at_startup(previous_launch_at_startup);
            }
            Err(settings_command_error(error))
        }
    }
}

#[tauri::command]
fn desktop_update_software_capture_shortcut(
    app: tauri::AppHandle,
    shortcut: String,
    settings: State<'_, Mutex<AppSettingsStore>>,
) -> Result<AppSettings, CommandError> {
    let shortcut = shortcut::normalize_global_shortcut(&shortcut)?;
    let mut settings = settings
        .lock()
        .map_err(|_| CommandError::new("settings.unavailable"))?;
    let previous: Box<str> = settings.software_capture_shortcut().into();
    let shortcut = software::rebind_software_capture_shortcut(&app, &shortcut)?;
    match settings.update_software_capture_shortcut(shortcut) {
        Ok(saved) => Ok(saved),
        Err(error) => {
            let _ = software::rebind_software_capture_shortcut(&app, &previous);
            Err(settings_command_error(error))
        }
    }
}

#[tauri::command]
fn desktop_privilege_status() -> Result<DesktopPrivilegeStatus, CommandError> {
    current_process_is_elevated()
        .map(|elevated| DesktopPrivilegeStatus { elevated })
        .map_err(|_| CommandError::new("settings.privilege_unavailable"))
}

#[tauri::command]
fn desktop_restart_elevated(app: tauri::AppHandle) -> Result<(), CommandError> {
    if current_process_is_elevated().unwrap_or(false) {
        return Ok(());
    }
    launch_current_process_elevated()
        .map_err(|_| CommandError::new("settings.elevation_failed"))?;
    app.exit(0);
    Ok(())
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
fn desktop_refresh_font_families(
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<DesktopProductSnapshot, CommandError> {
    application
        .lock()
        .map_err(|_| workspace_unavailable())?
        .refresh_font_families()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init());
    #[cfg(windows)]
    let builder = builder.plugin(
        tauri_plugin_global_shortcut::Builder::new()
            .with_handler(|app, shortcut, event| {
                workflow_shortcut::handle(app, shortcut, event.state());
                if event.state() == tauri_plugin_global_shortcut::ShortcutState::Pressed
                    && !workflow_shortcut::recording(app)
                    && software::matches_software_quick_capture_shortcut(app, shortcut)
                {
                    software::handle_software_quick_capture_shortcut(app);
                }
            })
            .build(),
    );
    builder
        .setup(|app| {
            let launch_context = DesktopLaunchContext::current();
            let data_root = launch_context.data_root.map_or_else(
                || {
                    app.path()
                        .data_dir()
                        .map(|path| default_workspace_root(&path))
                        .map_err(|error| std::io::Error::other(error.to_string()))
                },
                Ok,
            )?;
            let runtime_root = launch_context
                .runtime_root
                .unwrap_or(app.path().resource_dir()?.join("runtime"));
            let mut settings = AppSettingsStore::open(&data_root)
                .map_err(|error| std::io::Error::other(format!("settings startup: {error:?}")))?;
            if settings.current().is_ok_and(|current| {
                current.should_request_elevation(current_process_is_elevated().ok())
            }) && launch_current_process_elevated().is_ok()
            {
                app.handle().exit(0);
                return Ok(());
            }
            let ai_state = ai::DesktopAiState::open(&data_root).map_err(std::io::Error::other)?;
            let saved_settings = settings.current().map_err(|error| std::io::Error::other(format!("settings startup: {error:?}")))?;
            let application =
                DesktopApplication::open(data_root, runtime_root, &saved_settings).map_err(std::io::Error::other)?;
            settings.initialize_favorite_fonts(&application.font_families)
                .map_err(|error| std::io::Error::other(format!("favorite fonts startup: {error:?}")))?;
            app.manage(Mutex::new(settings));
            app.manage(Mutex::new(ai_state));
            app.manage(Mutex::new(application));
            software::manage_quick_capture(app);
            workflow_shortcut::manage(app);
            window_controls::setup(app.handle(), &saved_settings)?;
            Ok(())
        })
        .on_window_event(window_controls::window_event)
        .invoke_handler(tauri::generate_handler![
            window_controls::desktop_hide_to_tray,
            window_controls::desktop_minimize_window,
            workflow_shortcut::desktop_set_shortcut_recording,
            workflow_shortcut::desktop_workflow_shortcut_errors,
            desktop_status,
            desktop_settings,
            updates::desktop_check_update,
            desktop_update_settings,
            settings::desktop_validate_regex_rule,
            settings::desktop_test_regex_rule,
            desktop_update_software_capture_shortcut,
            desktop_privilege_status,
            desktop_restart_elevated,
            desktop_snapshot,
            desktop_refresh_font_families,
            data::desktop_open_dictionary_directory,
            ai::desktop_ai_profiles,
            ai_models::desktop_ai_models,
            ai::desktop_save_ai_profile,
            ai::desktop_set_default_ai_profile,
            ai::desktop_delete_ai_profile,
            ai::desktop_plan_ai_translation,
            ai::desktop_plan_probe_ai_translation,
            ai::desktop_apply_probe_ai_results,
            ai::desktop_start_ai_translation,
            ai::desktop_ai_translation_job,
            ai::desktop_ai_translation_tasks,
            ai::desktop_cancel_ai_translation,
            probe::desktop_probe_runs,
            probe::desktop_compatible_probe_adapters,
            probe::desktop_clear_probe_run_entries,
            probe::desktop_resume_probe_run,
            probe::desktop_set_probe_run_paused,
            probe::desktop_disconnect_probe_run,
            probe::desktop_refresh_probe_text,
            probe::desktop_probe_run_summary,
            probe::desktop_probe_run_entries,
            probe::desktop_edit_probe_translation,
            probe::desktop_sync_probe_dictionary_entries,
            probe::desktop_bulk_probe_entries,
            probe::desktop_export_probe_run,
            probe_transfer::desktop_import_probe_entries,
            probe_transfer::desktop_preview_dictionary_import,
            dictionary::desktop_dictionary,
            dictionary::desktop_query_dictionary_catalog,
            dictionary::desktop_install_dictionary_release,
            dictionary::desktop_create_dictionary,
            dictionary::desktop_import_dictionary,
            dictionary::desktop_export_dictionary,
            dictionary::desktop_update_dictionary,
            dictionary::desktop_delete_dictionaries,
            workflow::desktop_workflow,
            workflow_collection::desktop_workflow_collection,
            workflow::desktop_create_workflow,
            workflow::desktop_update_workflow,
            workflow::desktop_copy_workflow,
            workflow::desktop_delete_workflows,
            software::desktop_preflight_software,
            software::desktop_running_software_targets,
            software::desktop_arm_software_capture,
            software::desktop_cancel_software_capture,
            software::desktop_probe_software_capture_shortcut,
            software::desktop_add_software,
            software::desktop_update_software,
            software::desktop_select_software,
            software::desktop_launch_software,
            exit::desktop_prepare_exit,
            workflow::desktop_enable_workflow,
            workflow::desktop_disable_workflow,
            workflow::desktop_refresh_workflows,
            workflow_collection::desktop_collect_workflow_sources,
            workflow_collection::desktop_set_workflow_collection,
            workflow::desktop_control_workflow_diagnostics,
            workflow::desktop_workflow_diagnostics,
            software::desktop_remove_software
        ])
        .build(tauri::generate_context!())
        .expect("failed to build Glyphshift desktop shell")
        .run(|app, event| {
            if let tauri::RunEvent::ExitRequested { api, .. } = event {
                if let Some(state) = app.try_state::<Mutex<DesktopApplication>>() {
                    let result = state.lock().map_err(|_| workspace_unavailable()).and_then(|mut application| application.prepare_exit());
                    if let Err(error) = result {
                        api.prevent_exit();
                        let _ = app.emit("glyphshift-exit-failed", error);
                    }
                }
            }
        });
}

#[cfg(test)]
#[path = "tests/mod.rs"]
mod tests;

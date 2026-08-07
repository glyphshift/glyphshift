mod acquisition;
mod command_error;
mod dictionary;
mod font_catalog;
mod interactive_translation;
mod probe;
mod quick_probe;
mod settings;
mod shortcut;
mod software;
mod workflow;

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
    ProbeRunSummary, ProbeRunUpdate, DEFAULT_MAX_ENTRIES,
};
use glyphshift_controller_windows::{
    current_process_is_elevated, foreground_windows_executable, foreground_windows_point,
    inspect_windows_executable, launch_process_elevated, WindowsElevationError, WindowsExecutable,
};
use glyphshift_desktop_backend::{
    BackendError, DesktopBackend, DesktopEnvironment, DesktopSnapshot, DictionaryCreate,
    DictionaryEdit, DictionaryEntryCreate, DictionaryView, EffectiveWorkflowIntent,
    ExecutableSelection, SoftwareEdit, WorkflowCreate, WorkflowEdit, WorkflowView,
};
use glyphshift_desktop_runtime::{
    AcquisitionResult, DesktopAcquisitionCancellation, DesktopAcquisitionError, DesktopPoint,
    DesktopRect, DesktopRuntimeError, DesktopRuntimePool, DesktopRuntimeStatus,
    HostOperationFailure, RuntimeBundle, RuntimeTraceBatch, RuntimeTraceRecord,
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
    DEFAULT_INTERACTIVE_TRANSLATION_SHORTCUT, DEFAULT_SOFTWARE_CAPTURE_SHORTCUT,
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
use workflow::{workflow_activation_command_error, WorkflowTargetRuntimeView};

const DESKTOP_API_VERSION: u16 = 24;
const DATA_ROOT_ARGUMENT: &str = "--glyphshift-data-root";
const RUNTIME_ROOT_ARGUMENT: &str = "--glyphshift-runtime-root";

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
                    architecture == "x86_64"
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
    fn supports_acquisition_adapter(&self, adapter_id: &str) -> bool;

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

    fn acquire_point(
        &mut self,
        software_id: &str,
        spec: &glyphshift_desktop_backend::DesktopRuntimeSpec,
        target_id: u64,
        adapter_id: &str,
        point: DesktopPoint,
        cancellation: &DesktopAcquisitionCancellation,
    ) -> Result<AcquisitionResult, DesktopAcquisitionError>;

    fn acquire_primary_point(
        &mut self,
        software_id: &str,
        spec: &glyphshift_desktop_backend::DesktopRuntimeSpec,
        adapter_id: &str,
        point: DesktopPoint,
        cancellation: &DesktopAcquisitionCancellation,
    ) -> Result<AcquisitionResult, DesktopAcquisitionError>;

    fn acquire_primary_region(
        &mut self,
        software_id: &str,
        spec: &glyphshift_desktop_backend::DesktopRuntimeSpec,
        adapter_id: &str,
        region: DesktopRect,
        cancellation: &DesktopAcquisitionCancellation,
    ) -> Result<AcquisitionResult, DesktopAcquisitionError>;

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
    fn supports_acquisition_adapter(&self, adapter_id: &str) -> bool {
        DesktopRuntimePool::supports_acquisition_adapter(self, adapter_id)
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

    fn acquire_point(
        &mut self,
        software_id: &str,
        spec: &glyphshift_desktop_backend::DesktopRuntimeSpec,
        target_id: u64,
        adapter_id: &str,
        point: DesktopPoint,
        cancellation: &DesktopAcquisitionCancellation,
    ) -> Result<AcquisitionResult, DesktopAcquisitionError> {
        DesktopRuntimePool::acquire_point(
            self,
            software_id,
            spec,
            target_id,
            adapter_id,
            point,
            cancellation,
        )
    }

    fn acquire_primary_point(
        &mut self,
        software_id: &str,
        spec: &glyphshift_desktop_backend::DesktopRuntimeSpec,
        adapter_id: &str,
        point: DesktopPoint,
        cancellation: &DesktopAcquisitionCancellation,
    ) -> Result<AcquisitionResult, DesktopAcquisitionError> {
        DesktopRuntimePool::acquire_primary_point(
            self,
            software_id,
            spec,
            adapter_id,
            point,
            cancellation,
        )
    }

    fn acquire_primary_region(
        &mut self,
        software_id: &str,
        spec: &glyphshift_desktop_backend::DesktopRuntimeSpec,
        adapter_id: &str,
        region: DesktopRect,
        cancellation: &DesktopAcquisitionCancellation,
    ) -> Result<AcquisitionResult, DesktopAcquisitionError> {
        DesktopRuntimePool::acquire_primary_region(
            self,
            software_id,
            spec,
            adapter_id,
            region,
            cancellation,
        )
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
    adapter_target_support: BTreeMap<Box<str>, AdapterTargetSupport>,
    font_families: Vec<Box<str>>,
    font_cache_root: PathBuf,
    probe_runs: ProbeRunStore,
    quick_probe_sessions: QuickProbeSessionStore,
    active_probe_run_id: Option<Box<str>>,
    active_probe_capability: Option<ProbeRuntimeCapability>,
}

impl DesktopApplication {
    fn open(data_root: PathBuf, runtime_root: PathBuf) -> Result<Self, String> {
        let probe_runs = ProbeRunStore::open(data_root.join("probe-runs"))
            .map_err(|error| format!("probe run startup: {error:?}"))?;
        let quick_probe_sessions = QuickProbeSessionStore::open(&data_root)
            .map_err(|error| format!("quick probe startup: {error:?}"))?;
        let dictionary_distribution = offline_dictionary_distribution(&data_root)?;
        let runtime_bundle = RuntimeBundle::open(runtime_root).ok();
        let adapter_target_support = runtime_bundle
            .as_ref()
            .into_iter()
            .flat_map(|bundle| bundle.adapter_options())
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
                    })
                    .collect()
            })
            .unwrap_or_default();
        let font_families = font_catalog::load(&data_root);
        let environment = DesktopEnvironment::new(
            runtime_bundle
                .as_ref()
                .map(|bundle| bundle.adapter_requirements().to_vec())
                .unwrap_or_default(),
            font_families.iter().cloned(),
        );
        let backend = DesktopBackend::open_with_environment(&data_root, environment)
            .map_err(|error| format!("{error:?}"))?;
        let mut application = Self {
            backend,
            dictionary_distribution,
            runtimes: runtime_bundle.map(|bundle| {
                Box::new(DesktopRuntimePool::new(bundle)) as Box<dyn WorkflowRuntimeService>
            }),
            workflow_runtime_status: BTreeMap::new(),
            adapters,
            adapter_target_support,
            font_families,
            font_cache_root: data_root.clone(),
            probe_runs,
            quick_probe_sessions,
            active_probe_run_id: None,
            active_probe_capability: None,
        };
        application
            .recover_quick_probe_sessions()
            .map_err(|error| format!("quick probe recovery: {error:?}"))?;
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

    fn refresh_font_families(&mut self) -> Result<DesktopProductSnapshot, CommandError> {
        let font_families = font_catalog::refresh(&self.font_cache_root)
            .map_err(|_| CommandError::new("font.cache_write_failed"))?;
        self.backend
            .replace_font_families(font_families.iter().cloned());
        self.font_families = font_families;
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
    mut update: AppSettingsUpdate,
    settings: State<'_, Mutex<AppSettingsStore>>,
) -> Result<AppSettings, CommandError> {
    let interactive_translation_shortcut =
        shortcut::normalize_global_shortcut(update.interactive_translation_shortcut())?;
    let software_capture_shortcut =
        shortcut::normalize_global_shortcut(update.software_capture_shortcut())?;
    update.set_interactive_translation_shortcut(interactive_translation_shortcut.clone());
    update.set_software_capture_shortcut(software_capture_shortcut.clone());
    let mut settings = settings
        .lock()
        .map_err(|_| CommandError::new("settings.unavailable"))?;
    if settings.interactive_translation_shortcut() != interactive_translation_shortcut.as_ref()
        || settings.software_capture_shortcut() != software_capture_shortcut.as_ref()
    {
        return Err(CommandError::new("settings.shortcut_update_failed"));
    }
    let previous_launch_at_startup = settings.launch_at_startup();
    let launch_at_startup_changed = previous_launch_at_startup != update.launch_at_startup();
    if launch_at_startup_changed {
        configure_launch_at_startup(update.launch_at_startup()).map_err(settings_command_error)?;
    }
    match settings.update(update) {
        Ok(saved) => Ok(saved),
        Err(error) => {
            if launch_at_startup_changed {
                let _ = configure_launch_at_startup(previous_launch_at_startup);
            }
            Err(settings_command_error(error))
        }
    }
}

#[tauri::command]
fn desktop_update_interactive_translation_shortcut(
    app: tauri::AppHandle,
    shortcut: String,
    settings: State<'_, Mutex<AppSettingsStore>>,
) -> Result<AppSettings, CommandError> {
    let shortcut = shortcut::normalize_global_shortcut(&shortcut)?;
    let mut settings = settings
        .lock()
        .map_err(|_| CommandError::new("settings.unavailable"))?;
    let previous: Box<str> = settings.interactive_translation_shortcut().into();
    let shortcut =
        interactive_translation::rebind_interactive_translation_shortcut(&app, &shortcut)?;
    match settings.update_interactive_translation_shortcut(shortcut) {
        Ok(saved) => Ok(saved),
        Err(error) => {
            let _ =
                interactive_translation::rebind_interactive_translation_shortcut(&app, &previous);
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
                if event.state() == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                    if software::matches_software_quick_capture_shortcut(app, shortcut) {
                        software::handle_software_quick_capture_shortcut(app);
                    } else if interactive_translation::matches_interactive_translation_shortcut(
                        app, shortcut,
                    ) {
                        interactive_translation::handle_interactive_translation_shortcut(app);
                    }
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
                        .app_data_dir()
                        .map(|path| path.join("workspace"))
                        .map_err(|error| std::io::Error::other(error.to_string()))
                },
                Ok,
            )?;
            let runtime_root = launch_context
                .runtime_root
                .unwrap_or(app.path().resource_dir()?.join("runtime"));
            let settings = AppSettingsStore::open(&data_root)
                .map_err(|error| std::io::Error::other(format!("settings startup: {error:?}")))?;
            if settings.current().is_ok_and(|current| {
                current.should_request_elevation(current_process_is_elevated().ok())
            }) && launch_current_process_elevated().is_ok()
            {
                app.handle().exit(0);
                return Ok(());
            }
            let application =
                DesktopApplication::open(data_root, runtime_root).map_err(std::io::Error::other)?;
            app.manage(Mutex::new(settings));
            app.manage(Mutex::new(application));
            app.manage(acquisition::AcquisitionCommandState::default());
            software::manage_quick_capture(app);
            interactive_translation::manage_interactive_translation(app);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            desktop_status,
            desktop_settings,
            desktop_update_settings,
            desktop_update_interactive_translation_shortcut,
            desktop_update_software_capture_shortcut,
            desktop_privilege_status,
            desktop_restart_elevated,
            desktop_snapshot,
            desktop_refresh_font_families,
            acquisition::desktop_acquire_point,
            acquisition::desktop_cancel_point_acquisition,
            interactive_translation::desktop_arm_interactive_translation,
            interactive_translation::desktop_cancel_interactive_translation,
            interactive_translation::desktop_probe_interactive_translation_shortcut,
            interactive_translation::desktop_interactive_translation_capabilities,
            interactive_translation::desktop_interactive_translation_bubble,
            interactive_translation::desktop_dismiss_interactive_translation_bubble,
            probe::desktop_probe_runs,
            probe::desktop_compatible_probe_adapters,
            probe::desktop_create_probe_run,
            probe::desktop_delete_probe_runs,
            probe::desktop_update_probe_run,
            probe::desktop_clear_probe_run_entries,
            probe::desktop_resume_probe_run,
            probe::desktop_set_probe_run_paused,
            probe::desktop_disconnect_probe_run,
            probe::desktop_probe_run_summary,
            probe::desktop_probe_run_entries,
            probe::desktop_edit_probe_translation,
            probe::desktop_bulk_probe_entries,
            probe::desktop_export_probe_run,
            quick_probe::desktop_create_probe_from_sources,
            quick_probe::desktop_retain_quick_probe,
            quick_probe::desktop_cleanup_quick_probe,
            dictionary::desktop_dictionary,
            dictionary::desktop_query_dictionary_catalog,
            dictionary::desktop_install_dictionary_release,
            dictionary::desktop_create_dictionary,
            dictionary::desktop_import_dictionary,
            dictionary::desktop_export_dictionary,
            dictionary::desktop_update_dictionary,
            dictionary::desktop_delete_dictionaries,
            workflow::desktop_workflow,
            workflow::desktop_create_workflow,
            workflow::desktop_update_workflow,
            workflow::desktop_copy_workflow,
            workflow::desktop_delete_workflows,
            software::desktop_preflight_software,
            software::desktop_arm_software_capture,
            software::desktop_cancel_software_capture,
            software::desktop_probe_software_capture_shortcut,
            software::desktop_add_software,
            software::desktop_update_software,
            software::desktop_select_software,
            workflow::desktop_enable_workflow,
            workflow::desktop_disable_workflow,
            workflow::desktop_refresh_workflows,
            workflow::desktop_control_workflow_diagnostics,
            workflow::desktop_workflow_diagnostics,
            software::desktop_remove_software
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Glyphshift desktop shell");
}

#[cfg(test)]
#[path = "tests/mod.rs"]
mod tests;

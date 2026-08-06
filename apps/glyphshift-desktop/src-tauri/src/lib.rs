mod command_error;
mod dictionary;
mod font_catalog;
mod probe;
mod settings;
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
    current_process_is_elevated, foreground_windows_executable, inspect_windows_executable,
    launch_process_elevated, WindowsElevationError, WindowsExecutable,
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
use probe::ProbeRuntimeCapability;
#[cfg(test)]
use probe::{
    capture_preview_publish_error, ProbeDictionaryBindingRequest, ProbeRunCreateRequest,
    ProbeRunUpdateRequest, ProbeTranslationEditRequest,
};
use serde::{Deserialize, Serialize};
use settings::{
    configure_launch_at_startup, AppSettings, AppSettingsStore, AppSettingsUpdate, SettingsError,
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

const DESKTOP_API_VERSION: u16 = 19;
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
    ) -> Result<ProbeRuntimeCapability, DesktopRuntimeError>;

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
    font_cache_root: PathBuf,
    probe_runs: ProbeRunStore,
    active_probe_run_id: Option<Box<str>>,
    active_probe_capability: Option<ProbeRuntimeCapability>,
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
            font_families,
            font_cache_root: data_root.clone(),
            probe_runs,
            active_probe_run_id: None,
            active_probe_capability: None,
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
    update: AppSettingsUpdate,
    settings: State<'_, Mutex<AppSettingsStore>>,
) -> Result<AppSettings, CommandError> {
    let mut settings = settings
        .lock()
        .map_err(|_| CommandError::new("settings.unavailable"))?;
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
            .with_handler(|app, _shortcut, event| {
                if event.state() == tauri_plugin_global_shortcut::ShortcutState::Pressed {
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
            software::manage_quick_capture(app);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            desktop_status,
            desktop_settings,
            desktop_update_settings,
            desktop_privilege_status,
            desktop_restart_elevated,
            desktop_snapshot,
            desktop_refresh_font_families,
            probe::desktop_probe_runs,
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

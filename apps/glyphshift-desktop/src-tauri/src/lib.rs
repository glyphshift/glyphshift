mod command_error;
mod settings;

use command_error::CommandError;
use glyphshift_capture::{
    unix_time_millis, CaptureCatalog, CaptureConfiguration, CaptureSessionId, DictionaryDraft,
    DEFAULT_MAX_ENTRIES,
};
use glyphshift_desktop_backend::{
    BackendError, DesktopBackend, DesktopEnvironment, DesktopSnapshot, DictionaryCreate,
    DictionaryEdit, DictionaryView, EffectiveWorkflowIntent, ExecutableSelection,
    FontProfileCreate, FontProfileEdit, FontProfileView, SoftwareEdit, WorkflowCreate,
    WorkflowEdit, WorkflowView,
};
use glyphshift_desktop_runtime::{
    DesktopRuntimeError, DesktopRuntimePool, DesktopRuntimeStatus, RuntimeBundle,
    WorkflowReconcileReport,
};
use glyphshift_domain::Feature;
use glyphshift_workflow::ResolveError;
use serde::Serialize;
use settings::{AppSettings, AppSettingsStore, AppSettingsUpdate, SettingsError};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{Manager, State};

const DESKTOP_API_VERSION: u16 = 9;
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
        BackendError::WorkflowRejected(ResolveError::UnknownLocation {
            software_id,
            location,
            ..
        }) => CommandError::new("workflow.unknown_location")
            .with_arg("softwareId", software_id.to_string())
            .with_arg("location", location.to_string()),
        BackendError::WorkflowRejected(ResolveError::UnknownSoftware(id))
        | BackendError::UnknownSoftware(id) => {
            CommandError::new("workflow.unknown_software").with_arg("softwareId", id.to_string())
        }
        BackendError::WorkflowRejected(ResolveError::UnknownDictionary(id))
        | BackendError::UnknownDictionary(id) => CommandError::new("workflow.unknown_dictionary")
            .with_arg("dictionaryId", id.to_string()),
        BackendError::WorkflowRejected(ResolveError::UnknownFontProfile(id))
        | BackendError::UnknownFontProfile(id) => {
            CommandError::new("workflow.unknown_font_profile")
                .with_arg("fontProfileId", id.to_string())
        }
        BackendError::WorkflowRejected(ResolveError::UnknownAdapter(id)) => {
            CommandError::new("workflow.unknown_adapter").with_arg("adapterId", id.to_string())
        }
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
    capture: Option<CaptureSummaryView>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct CaptureSummaryView {
    session_id: Box<str>,
    software_id: Box<str>,
    adapter_ids: Vec<Box<str>>,
    status: &'static str,
    entry_count: usize,
    dropped_observations: u64,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct CaptureResultView {
    catalog: CaptureCatalog,
    dictionary_draft: DictionaryDraft,
}

struct CaptureState {
    summary: CaptureSummaryView,
    output_path: PathBuf,
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
}

struct DesktopApplication {
    backend: DesktopBackend,
    runtimes: Option<Box<dyn WorkflowRuntimeService>>,
    workflow_runtime_status: BTreeMap<Box<str>, WorkflowRuntimeView>,
    adapters: Vec<AdapterView>,
    font_families: Vec<Box<str>>,
    capture_root: PathBuf,
    capture: Option<CaptureState>,
}

impl DesktopApplication {
    fn open(data_root: PathBuf, runtime_root: PathBuf) -> Result<Self, String> {
        let capture_root = data_root.join("captures");
        std::fs::create_dir_all(&capture_root).map_err(|error| error.to_string())?;
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
            runtimes: runtime_bundle.map(|bundle| {
                Box::new(DesktopRuntimePool::new(bundle)) as Box<dyn WorkflowRuntimeService>
            }),
            workflow_runtime_status: BTreeMap::new(),
            adapters,
            font_families,
            capture_root,
            capture: None,
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
            capture: self.capture.as_ref().map(|capture| capture.summary.clone()),
        }
    }

    fn start_capture(
        &mut self,
        software_id: &str,
        adapter_ids: Vec<Box<str>>,
    ) -> Result<DesktopProductSnapshot, CommandError> {
        if self
            .capture
            .as_ref()
            .is_some_and(|capture| capture.summary.status == "active")
        {
            return Err(CommandError::new("capture.already_active"));
        }
        let spec = self
            .backend
            .capture_runtime_spec(software_id, &adapter_ids)
            .map_err(capture_backend_error)?;
        let runtimes = self
            .runtimes
            .as_mut()
            .ok_or_else(|| CommandError::new("runtime.unavailable"))?;
        let timestamp = unix_time_millis();
        let mut suffix = 0_u32;
        let (session_id, output_path) = loop {
            let id = if suffix == 0 {
                format!("capture-{timestamp}")
            } else {
                format!("capture-{timestamp}-{suffix}")
            };
            let path = self.capture_root.join(format!("{id}.json"));
            if !path.exists() {
                break (id, path);
            }
            suffix = suffix.saturating_add(1);
        };
        let configuration = CaptureConfiguration::new(
            CaptureSessionId::new(session_id.clone())
                .map_err(|_| CommandError::new("capture.invalid_configuration"))?,
            output_path.clone(),
            DEFAULT_MAX_ENTRIES,
        )
        .map_err(|_| CommandError::new("capture.invalid_configuration"))?;
        runtimes
            .start_capture(software_id, &spec, configuration)
            .map_err(|error| runtime_command_error(error, true))?;
        self.capture = Some(CaptureState {
            summary: CaptureSummaryView {
                session_id: session_id.into(),
                software_id: software_id.into(),
                adapter_ids,
                status: "active",
                entry_count: 0,
                dropped_observations: 0,
            },
            output_path,
        });
        Ok(self.snapshot())
    }

    fn stop_capture(&mut self) -> Result<DesktopProductSnapshot, CommandError> {
        let capture = self
            .capture
            .as_mut()
            .filter(|capture| capture.summary.status == "active")
            .ok_or_else(|| CommandError::new("capture.not_active"))?;
        self.runtimes
            .as_mut()
            .ok_or_else(|| CommandError::new("runtime.unavailable"))?
            .stop_capture(&capture.summary.software_id)
            .map_err(|error| runtime_command_error(error, false))?;
        let catalog = CaptureCatalog::read(&capture.output_path)
            .map_err(|_| CommandError::new("capture.read_failed"))?;
        let draft = catalog.dictionary_draft();
        let draft_path = capture.output_path.with_extension("dictionary-draft.json");
        std::fs::write(
            draft_path,
            draft
                .encode_json()
                .map_err(|_| CommandError::new("capture.write_failed"))?,
        )
        .map_err(|_| CommandError::new("capture.write_failed"))?;
        capture.summary.status = "completed";
        capture.summary.entry_count = catalog.entries().len();
        capture.summary.dropped_observations = catalog.dropped_observations();
        Ok(self.snapshot())
    }

    fn capture_result(&self) -> Result<CaptureResultView, CommandError> {
        let capture = self
            .capture
            .as_ref()
            .filter(|capture| capture.summary.status == "completed")
            .ok_or_else(|| CommandError::new("capture.not_completed"))?;
        let catalog = CaptureCatalog::read(&capture.output_path)
            .map_err(|_| CommandError::new("capture.read_failed"))?;
        let dictionary_draft = catalog.dictionary_draft();
        Ok(CaptureResultView {
            catalog,
            dictionary_draft,
        })
    }

    fn dictionary_detail(&self, dictionary_id: &str) -> Result<DictionaryView, CommandError> {
        self.backend
            .dictionary(dictionary_id)
            .cloned()
            .map_err(|_| {
                CommandError::new("dictionary.not_found").with_arg("dictionaryId", dictionary_id)
            })
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
        self.backend
            .delete_dictionaries(dictionary_ids.iter().map(AsRef::as_ref))
            .map_err(|_| CommandError::new("dictionary.referenced"))?;
        Ok(self.snapshot())
    }

    fn font_profile_detail(&self, font_profile_id: &str) -> Result<FontProfileView, CommandError> {
        self.backend
            .font_profile(font_profile_id)
            .cloned()
            .map_err(|_| {
                CommandError::new("font_profile.not_found")
                    .with_arg("fontProfileId", font_profile_id)
            })
    }

    fn create_font_profile(
        &mut self,
        create: FontProfileCreate,
    ) -> Result<DesktopProductSnapshot, CommandError> {
        self.backend
            .create_font_profile(create)
            .map_err(|_| CommandError::new("font_profile.invalid_create"))?;
        Ok(self.snapshot())
    }

    fn update_font_profile(
        &mut self,
        edit: FontProfileEdit,
    ) -> Result<DesktopProductSnapshot, CommandError> {
        self.backend
            .update_font_profile(edit)
            .map_err(|_| CommandError::new("font_profile.invalid_update"))?;
        self.reconcile_enabled_workflows()?;
        Ok(self.snapshot())
    }

    fn delete_font_profiles(
        &mut self,
        font_profile_ids: &[Box<str>],
    ) -> Result<DesktopProductSnapshot, CommandError> {
        self.backend
            .delete_font_profiles(font_profile_ids.iter().map(AsRef::as_ref))
            .map_err(|_| CommandError::new("font_profile.referenced"))?;
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
        _ if enabling => CommandError::new("runtime.activation_failed"),
        _ => CommandError::new("runtime.stop_unconfirmed"),
    }
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
fn desktop_start_capture(
    software_id: String,
    adapter_ids: Vec<String>,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<DesktopProductSnapshot, CommandError> {
    application
        .lock()
        .map_err(|_| runtime_unavailable())?
        .start_capture(
            &software_id,
            adapter_ids.into_iter().map(Box::<str>::from).collect(),
        )
}

#[tauri::command]
fn desktop_stop_capture(
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<DesktopProductSnapshot, CommandError> {
    application
        .lock()
        .map_err(|_| runtime_unavailable())?
        .stop_capture()
}

#[tauri::command]
fn desktop_capture_result(
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<CaptureResultView, CommandError> {
    application
        .lock()
        .map_err(|_| workspace_unavailable())?
        .capture_result()
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
fn desktop_font_profile(
    font_profile_id: String,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<FontProfileView, CommandError> {
    application
        .lock()
        .map_err(|_| workspace_unavailable())?
        .font_profile_detail(&font_profile_id)
}

#[tauri::command]
fn desktop_create_font_profile(
    create: FontProfileCreate,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<DesktopProductSnapshot, CommandError> {
    application
        .lock()
        .map_err(|_| workspace_unavailable())?
        .create_font_profile(create)
}

#[tauri::command]
fn desktop_update_font_profile(
    edit: FontProfileEdit,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<DesktopProductSnapshot, CommandError> {
    application
        .lock()
        .map_err(|_| workspace_unavailable())?
        .update_font_profile(edit)
}

#[tauri::command]
fn desktop_delete_font_profiles(
    font_profile_ids: Vec<String>,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<DesktopProductSnapshot, CommandError> {
    let font_profile_ids = font_profile_ids
        .into_iter()
        .map(Box::<str>::from)
        .collect::<Vec<_>>();
    application
        .lock()
        .map_err(|_| workspace_unavailable())?
        .delete_font_profiles(&font_profile_ids)
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
            desktop_start_capture,
            desktop_stop_capture,
            desktop_capture_result,
            desktop_dictionary,
            desktop_create_dictionary,
            desktop_update_dictionary,
            desktop_delete_dictionaries,
            desktop_font_profile,
            desktop_create_font_profile,
            desktop_update_font_profile,
            desktop_delete_font_profiles,
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
    use std::fs;
    use std::sync::{Arc, Mutex as StdMutex};
    use tempfile::tempdir;

    const TEST_ADAPTER_ID: &str = "test.inline";

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
                runtimes: Some(runtimes),
                workflow_runtime_status: BTreeMap::new(),
                adapters: Vec::new(),
                font_families: Vec::new(),
                capture_root: data_root.path().join("captures"),
                capture: None,
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
    fn capture_commands_generate_a_private_catalog_and_a_pure_dictionary_draft() {
        let (mut application, calls, software_id, _data_root) = workflow_application();

        let started = application
            .start_capture(&software_id, vec![TEST_ADAPTER_ID.into()])
            .expect("start product capture");
        let capture = started.capture.expect("active capture summary");
        assert_eq!(capture.status, "active");
        assert_eq!(capture.software_id, software_id);

        let stopped = application.stop_capture().expect("stop product capture");
        assert_eq!(
            stopped.capture.as_ref().map(|capture| capture.status),
            Some("completed")
        );
        let result = application.capture_result().expect("capture result");
        assert!(result.catalog.entries().is_empty());
        assert!(result.dictionary_draft.entries().is_empty());
        let serialized = serde_json::to_value(stopped).expect("serialize capture snapshot");
        assert!(serialized.get("outputPath").is_none());
        assert_eq!(
            calls.lock().expect("runtime call log").captures_started,
            vec![software_id.clone()]
        );
        assert_eq!(
            calls.lock().expect("runtime call log").captures_stopped,
            vec![software_id]
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
            runtimes: Some(runtimes),
            workflow_runtime_status: BTreeMap::new(),
            adapters: Vec::new(),
            font_families: Vec::new(),
            capture_root: data_root.path().join("captures"),
            capture: None,
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

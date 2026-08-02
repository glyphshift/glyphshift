use glyphshift_desktop_backend::{
    BackendError, DesktopBackend, DesktopSnapshot, DictionaryCreate, DictionaryEdit,
    DictionaryView, EffectiveWorkflowIntent, ExecutableSelection, SoftwareEdit, WorkflowCreate,
    WorkflowEdit, WorkflowView,
};
use glyphshift_desktop_runtime::{
    DesktopRuntimeError, DesktopRuntimePool, DesktopRuntimeStatus, RuntimeBundle,
    WorkflowReconcileReport,
};
use glyphshift_domain::Feature;
use glyphshift_workflow::ResolveError;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{Manager, State};

const DESKTOP_API_VERSION: u16 = 5;
const WINDOWS_FONT_REGISTRY_KEY: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Fonts";

fn workflow_activation_error_message(error: BackendError) -> String {
    match error {
        BackendError::WorkflowRejected(ResolveError::NoEffectiveRules { .. }) => {
            "无法启用工作流：词典没有可应用规则，请先添加文字替换或字体规则。".to_owned()
        }
        BackendError::WorkflowRejected(ResolveError::EmptyTarget { .. }) => {
            "无法启用工作流：至少需要为每个软件选择一份词典。".to_owned()
        }
        BackendError::WorkflowRejected(ResolveError::LocaleMismatch { .. }) => {
            "无法启用工作流：软件与词典的语言不一致。".to_owned()
        }
        BackendError::WorkflowRejected(ResolveError::UnknownLocation { .. }) => {
            "无法启用工作流：词典包含该软件不支持的位置。".to_owned()
        }
        BackendError::WorkflowRejected(ResolveError::UnknownSoftware(_))
        | BackendError::UnknownSoftware(_) => "无法启用工作流：引用的软件已不存在。".to_owned(),
        BackendError::WorkflowRejected(ResolveError::UnknownDictionary(_))
        | BackendError::UnknownDictionary(_) => "无法启用工作流：引用的词典已不存在。".to_owned(),
        BackendError::SoftwareOccupied { .. } => {
            "无法启用工作流：目标软件已被其他工作流占用，请先停用冲突工作流。".to_owned()
        }
        BackendError::Storage(_) => {
            "无法启用工作流：启用状态未能保存，请检查数据目录是否可写。".to_owned()
        }
        _ => "无法启用工作流：工作流配置无效。".to_owned(),
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
    errors: BTreeMap<Box<str>, String>,
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
struct HookTypeView {
    id: Box<str>,
    label: Box<str>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct DesktopProductSnapshot {
    #[serde(flatten)]
    configuration: DesktopSnapshot,
    workflow_runtime_status: BTreeMap<Box<str>, WorkflowRuntimeView>,
    hook_types: Vec<HookTypeView>,
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
}

struct DesktopApplication {
    backend: DesktopBackend,
    runtimes: Option<Box<dyn WorkflowRuntimeService>>,
    workflow_runtime_status: BTreeMap<Box<str>, WorkflowRuntimeView>,
    hook_types: Vec<HookTypeView>,
    font_families: Vec<Box<str>>,
}

impl DesktopApplication {
    fn open(data_root: PathBuf, runtime_root: PathBuf) -> Result<Self, String> {
        let runtime_bundle = RuntimeBundle::open(runtime_root).ok();
        let hook_types = runtime_bundle
            .as_ref()
            .map(|bundle| {
                bundle
                    .translation_adapter_options()
                    .iter()
                    .map(|adapter| HookTypeView {
                        id: adapter.id().into(),
                        label: adapter.label().into(),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let backend = DesktopBackend::open(data_root).map_err(|error| format!("{error:?}"))?;
        let mut application = Self {
            backend,
            runtimes: runtime_bundle.map(|bundle| {
                Box::new(DesktopRuntimePool::new(bundle)) as Box<dyn WorkflowRuntimeService>
            }),
            workflow_runtime_status: BTreeMap::new(),
            hook_types,
            font_families: system_font_families(),
        };
        application.restore_enabled_workflows()?;
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
            hook_types: self.hook_types.clone(),
            font_families: self.font_families.clone(),
        }
    }

    fn dictionary_detail(&self, dictionary_id: &str) -> Result<DictionaryView, String> {
        self.backend
            .dictionary(dictionary_id)
            .cloned()
            .map_err(|_| "没有找到这个词典".to_owned())
    }

    fn create_dictionary(
        &mut self,
        create: DictionaryCreate,
    ) -> Result<DesktopProductSnapshot, String> {
        self.backend
            .create_dictionary(create)
            .map_err(|_| "词典名称、标识或规则无效".to_owned())?;
        Ok(self.snapshot())
    }

    fn update_dictionary(
        &mut self,
        edit: DictionaryEdit,
    ) -> Result<DesktopProductSnapshot, String> {
        self.backend
            .update_dictionary(edit)
            .map_err(|_| "词典已变化或规则无效，请重新加载".to_owned())?;
        self.reconcile_enabled_workflows()?;
        Ok(self.snapshot())
    }

    fn delete_dictionaries(
        &mut self,
        dictionary_ids: &[Box<str>],
    ) -> Result<DesktopProductSnapshot, String> {
        self.backend
            .delete_dictionaries(dictionary_ids.iter().map(AsRef::as_ref))
            .map_err(|_| "被工作流引用的词典不能删除".to_owned())?;
        Ok(self.snapshot())
    }

    fn workflow_detail(&self, workflow_id: &str) -> Result<WorkflowView, String> {
        self.backend
            .workflow(workflow_id)
            .map_err(|_| "没有找到这个工作流".to_owned())
    }

    fn create_workflow(
        &mut self,
        create: WorkflowCreate,
    ) -> Result<DesktopProductSnapshot, String> {
        self.backend
            .create_workflow(create)
            .map_err(|_| "工作流名称、标识或目标无效".to_owned())?;
        Ok(self.snapshot())
    }

    fn update_workflow(&mut self, edit: WorkflowEdit) -> Result<DesktopProductSnapshot, String> {
        let workflow_id = self
            .backend
            .update_workflow(edit)
            .map_err(|_| "工作流已变化或定义无效，请重新加载".to_owned())?
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
    ) -> Result<DesktopProductSnapshot, String> {
        self.backend
            .copy_workflow(source_workflow_id, new_workflow_id, name)
            .map_err(|_| "工作流副本的名称或标识无效".to_owned())?;
        Ok(self.snapshot())
    }

    fn delete_workflows(
        &mut self,
        workflow_ids: &[Box<str>],
    ) -> Result<DesktopProductSnapshot, String> {
        self.backend
            .delete_workflows(workflow_ids.iter().map(AsRef::as_ref))
            .map_err(|_| "启用中的工作流不能删除，请先停用".to_owned())?;
        for workflow_id in workflow_ids {
            self.workflow_runtime_status.remove(workflow_id.as_ref());
        }
        Ok(self.snapshot())
    }

    fn reconcile_workflow_if_enabled(&mut self, workflow_id: &str) -> Result<(), String> {
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
            .map_err(|_| "工作流定义无效，未能更新运行状态".to_owned())?;
        let runtime = self.runtimes.as_mut().map_or_else(
            || unavailable_workflow_runtime_view(&intent, true),
            |runtimes| runtimes.activate_workflow(&intent, false),
        );
        self.workflow_runtime_status
            .insert(workflow_id.into(), runtime);
        Ok(())
    }

    fn reconcile_enabled_workflows(&mut self) -> Result<(), String> {
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

    fn restore_enabled_workflows(&mut self) -> Result<(), String> {
        self.reconcile_enabled_workflows()
    }

    fn refresh_workflows(&mut self) -> Result<DesktopProductSnapshot, String> {
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
                .map_err(|_| "工作流定义无效，未能刷新运行状态".to_owned())?;
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
    ) -> Result<WorkflowCommandResult, String> {
        let intent = self
            .backend
            .effective_workflow_intent(workflow_id)
            .map_err(workflow_activation_error_message)?;
        if replace_conflicts {
            self.backend
                .replace_workflow_activation(workflow_id)
                .map_err(workflow_activation_error_message)?;
        } else {
            self.backend
                .enable_workflow(workflow_id)
                .map_err(workflow_activation_error_message)?;
        }
        let runtime = self.runtimes.as_mut().map_or_else(
            || unavailable_workflow_runtime_view(&intent, true),
            |runtimes| runtimes.activate_workflow(&intent, replace_conflicts),
        );
        let definition = self
            .backend
            .workflow(workflow_id)
            .map_err(|_| "工作流定义无效".to_owned())?;
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

    fn disable_workflow(&mut self, workflow_id: &str) -> Result<WorkflowCommandResult, String> {
        let intent = self
            .backend
            .effective_workflow_intent(workflow_id)
            .map_err(|_| "工作流定义无效，未能停用".to_owned())?;
        self.backend
            .disable_workflow(workflow_id)
            .map_err(|_| "工作流未能停用".to_owned())?;
        let runtime = self.runtimes.as_mut().map_or_else(
            || unavailable_workflow_runtime_view(&intent, false),
            |runtimes| runtimes.stop_workflow(&intent),
        );
        let definition = self
            .backend
            .workflow(workflow_id)
            .map_err(|_| "工作流定义无效".to_owned())?;
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

    fn add_software(&mut self, executable_path: String) -> Result<DesktopProductSnapshot, String> {
        self.backend
            .add_software(ExecutableSelection::new(executable_path))
            .map_err(|_| "请选择一个可访问的 Windows 应用程序（.exe）".to_owned())?;
        Ok(self.snapshot())
    }

    fn select_software(&mut self, extension_id: &str) -> Result<DesktopProductSnapshot, String> {
        self.backend
            .select_software(extension_id)
            .map_err(|_| "软件选择未能保存".to_owned())?;
        Ok(self.snapshot())
    }

    fn remove_software(&mut self, extension_id: &str) -> Result<DesktopProductSnapshot, String> {
        if let Some(runtimes) = self.runtimes.as_mut() {
            runtimes
                .remove_software(extension_id)
                .map_err(|_| "目标进程未确认停止，软件没有删除".to_owned())?;
        }
        self.backend
            .remove_software(extension_id)
            .map_err(|_| "软件未能从 Glyphshift 中删除".to_owned())?;
        Ok(self.snapshot())
    }

    fn update_software(
        &mut self,
        extension_id: String,
        display_name: String,
        description: String,
        executable_path: String,
    ) -> Result<DesktopProductSnapshot, String> {
        let edit = SoftwareEdit::new(extension_id.clone(), display_name, executable_path)
            .with_description(description);
        self.backend
            .validate_software_edit(&edit)
            .map_err(|_| "软件名称或程序路径无效，请重新检查".to_owned())?;
        if let Some(runtimes) = self.runtimes.as_mut() {
            runtimes
                .remove_software(&extension_id)
                .map_err(|_| "目标进程未确认停止，软件信息没有修改".to_owned())?;
        }
        self.backend
            .update_software(edit)
            .map_err(|_| "软件名称或程序路径无效，请重新检查".to_owned())?;
        self.reconcile_enabled_workflows()?;
        Ok(self.snapshot())
    }
}

fn runtime_error_message(error: DesktopRuntimeError, enabling: bool) -> String {
    match error {
        DesktopRuntimeError::UnknownTarget => {
            "没有找到与该程序路径匹配的运行实例；请先启动这个版本的软件".to_owned()
        }
        DesktopRuntimeError::SessionRejected if enabling => {
            "已找到运行实例，但目标软件拒绝了功能激活；请重试，仍失败时重启目标软件".to_owned()
        }
        DesktopRuntimeError::BundleUnavailable => {
            "运行组件尚未准备好，请通过开发任务重新启动 Glyphshift".to_owned()
        }
        _ if enabling => "已找到运行实例，但翻译组件未能完成激活，功能未开启".to_owned(),
        _ => "目标进程未确认停止，功能状态没有改变".to_owned(),
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
                errors.insert(software_id.into(), runtime_error_message(error, enabling));
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
                    runtime_error_message(DesktopRuntimeError::BundleUnavailable, enabling),
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
) -> Result<DesktopProductSnapshot, String> {
    application
        .lock()
        .map_err(|_| "桌面工作区暂时不可用".to_owned())
        .map(|application| application.snapshot())
}

#[tauri::command]
fn desktop_dictionary(
    dictionary_id: String,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<DictionaryView, String> {
    application
        .lock()
        .map_err(|_| "桌面工作区暂时不可用".to_owned())?
        .dictionary_detail(&dictionary_id)
}

#[tauri::command]
fn desktop_create_dictionary(
    create: DictionaryCreate,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<DesktopProductSnapshot, String> {
    application
        .lock()
        .map_err(|_| "桌面工作区暂时不可用".to_owned())?
        .create_dictionary(create)
}

#[tauri::command]
fn desktop_update_dictionary(
    edit: DictionaryEdit,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<DesktopProductSnapshot, String> {
    application
        .lock()
        .map_err(|_| "桌面工作区暂时不可用".to_owned())?
        .update_dictionary(edit)
}

#[tauri::command]
fn desktop_delete_dictionaries(
    dictionary_ids: Vec<String>,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<DesktopProductSnapshot, String> {
    let dictionary_ids = dictionary_ids
        .into_iter()
        .map(Box::<str>::from)
        .collect::<Vec<_>>();
    application
        .lock()
        .map_err(|_| "桌面工作区暂时不可用".to_owned())?
        .delete_dictionaries(&dictionary_ids)
}

#[tauri::command]
fn desktop_workflow(
    workflow_id: String,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<WorkflowView, String> {
    application
        .lock()
        .map_err(|_| "桌面工作区暂时不可用".to_owned())?
        .workflow_detail(&workflow_id)
}

#[tauri::command]
fn desktop_create_workflow(
    create: WorkflowCreate,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<DesktopProductSnapshot, String> {
    application
        .lock()
        .map_err(|_| "桌面工作区暂时不可用".to_owned())?
        .create_workflow(create)
}

#[tauri::command]
fn desktop_update_workflow(
    edit: WorkflowEdit,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<DesktopProductSnapshot, String> {
    application
        .lock()
        .map_err(|_| "桌面工作区暂时不可用".to_owned())?
        .update_workflow(edit)
}

#[tauri::command]
fn desktop_copy_workflow(
    source_workflow_id: String,
    new_workflow_id: String,
    name: String,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<DesktopProductSnapshot, String> {
    application
        .lock()
        .map_err(|_| "桌面工作区暂时不可用".to_owned())?
        .copy_workflow(&source_workflow_id, new_workflow_id.into(), name.into())
}

#[tauri::command]
fn desktop_delete_workflows(
    workflow_ids: Vec<String>,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<DesktopProductSnapshot, String> {
    let workflow_ids = workflow_ids
        .into_iter()
        .map(Box::<str>::from)
        .collect::<Vec<_>>();
    application
        .lock()
        .map_err(|_| "桌面工作区暂时不可用".to_owned())?
        .delete_workflows(&workflow_ids)
}

#[tauri::command]
fn desktop_add_software(
    executable_path: String,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<DesktopProductSnapshot, String> {
    application
        .lock()
        .map_err(|_| "桌面工作区暂时不可用".to_owned())?
        .add_software(executable_path)
}

#[tauri::command]
fn desktop_select_software(
    extension_id: String,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<DesktopProductSnapshot, String> {
    application
        .lock()
        .map_err(|_| "桌面工作区暂时不可用".to_owned())?
        .select_software(&extension_id)
}

#[tauri::command]
fn desktop_update_software(
    extension_id: String,
    display_name: String,
    description: String,
    executable_path: String,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<DesktopProductSnapshot, String> {
    application
        .lock()
        .map_err(|_| "桌面工作区暂时不可用".to_owned())?
        .update_software(extension_id, display_name, description, executable_path)
}

#[tauri::command]
fn desktop_enable_workflow(
    workflow_id: String,
    replace_conflicts: bool,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<WorkflowCommandResult, String> {
    application
        .lock()
        .map_err(|_| "桌面运行服务暂时不可用".to_owned())?
        .enable_workflow(&workflow_id, replace_conflicts)
}

#[tauri::command]
fn desktop_disable_workflow(
    workflow_id: String,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<WorkflowCommandResult, String> {
    application
        .lock()
        .map_err(|_| "桌面运行服务暂时不可用".to_owned())?
        .disable_workflow(&workflow_id)
}

#[tauri::command]
fn desktop_refresh_workflows(
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<DesktopProductSnapshot, String> {
    application
        .lock()
        .map_err(|_| "桌面运行服务暂时不可用".to_owned())?
        .refresh_workflows()
}

#[tauri::command]
fn desktop_remove_software(
    extension_id: String,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<DesktopProductSnapshot, String> {
    application
        .lock()
        .map_err(|_| "桌面工作区暂时不可用".to_owned())?
        .remove_software(&extension_id)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_root = app
                .path()
                .app_data_dir()
                .map_err(|error| std::io::Error::other(error.to_string()))?
                .join("workspace");
            let runtime_root = std::env::var_os("GLYPHSHIFT_RUNTIME_ROOT")
                .map(PathBuf::from)
                .unwrap_or(app.path().resource_dir()?.join("runtime"));
            let application =
                DesktopApplication::open(data_root, runtime_root).map_err(std::io::Error::other)?;
            app.manage(Mutex::new(application));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            desktop_status,
            desktop_snapshot,
            desktop_dictionary,
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
            desktop_remove_software
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Glyphshift desktop shell");
}

#[cfg(test)]
mod tests {
    use super::*;
    use glyphshift_desktop_backend::{
        DictionaryCreate, DictionaryEdit, DictionaryRuleCreate, WorkflowCreate, WorkflowEdit,
        WorkflowTargetCreate,
    };
    use std::fs;
    use std::sync::{Arc, Mutex as StdMutex};
    use tempfile::tempdir;

    #[derive(Default)]
    struct WorkflowRuntimeCalls {
        enabled: Vec<(Box<str>, bool, Vec<Box<str>>)>,
        disabled: Vec<Box<str>>,
        refreshed: Vec<Box<str>>,
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
        let mut backend = DesktopBackend::open(data_root.path()).expect("open product backend");
        let software_id: Box<str> = backend
            .add_software(ExecutableSelection::new(&executable))
            .expect("add synthetic software")
            .software()[0]
            .id()
            .into();
        backend
            .create_dictionary(
                DictionaryCreate::new("dictionary.product", "产品词典", "zh-CN")
                    .with_entries([DictionaryRuleCreate::replace("main-ui", "Open", "打开")]),
            )
            .expect("create dictionary");
        backend
            .create_workflow(
                WorkflowCreate::new("workflow.product", "产品工作流").with_targets([
                    WorkflowTargetCreate::new(software_id.clone(), ["dictionary.product"]),
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
                hook_types: Vec::new(),
                font_families: Vec::new(),
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
        assert!(
            runtime_error_message(DesktopRuntimeError::UnknownTarget, true)
                .contains("程序路径匹配")
        );
        assert!(
            runtime_error_message(DesktopRuntimeError::SessionRejected, true)
                .contains("拒绝了功能激活")
        );
    }

    #[test]
    fn workflow_activation_error_explains_an_empty_dictionary() {
        assert_eq!(
            workflow_activation_error_message(BackendError::WorkflowRejected(
                ResolveError::NoEffectiveRules {
                    software_id: "software.empty".into(),
                },
            )),
            "无法启用工作流：词典没有可应用规则，请先添加文字替换或字体规则。"
        );
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
        assert_eq!(json["dictionaries"][0]["id"], "dictionary.product");
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
        assert_eq!(dictionary.entries()[0].translation(), Some("打开"));
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
                    WorkflowTargetCreate::new(software_id, ["dictionary.product"]),
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
                    WorkflowTargetCreate::new(software_id, ["dictionary.product"]),
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
                DictionaryCreate::new("dictionary.secondary", "备用词典", "zh-CN")
                    .with_entries([DictionaryRuleCreate::keep("main-ui", "Close")]),
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
                DictionaryEdit::new("dictionary.product", "产品词典 2", "zh-CN", 1)
                    .with_entries([DictionaryRuleCreate::replace("main-ui", "Open", "开启")]),
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
        let backend = DesktopBackend::open(data_root.path()).expect("reopen product backend");
        let calls = Arc::new(StdMutex::new(WorkflowRuntimeCalls::default()));
        let runtimes: Box<dyn WorkflowRuntimeService> = Box::new(RecordingWorkflowRuntime {
            calls: Arc::clone(&calls),
        });
        let mut reopened = DesktopApplication {
            backend,
            runtimes: Some(runtimes),
            workflow_runtime_status: BTreeMap::new(),
            hook_types: Vec::new(),
            font_families: Vec::new(),
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

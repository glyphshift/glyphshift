use super::*;

pub(super) fn workflow_activation_command_error(error: BackendError) -> CommandError {
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
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(super) struct WorkflowTargetRuntimeView {
    pub(super) software_id: Box<str>,
    pub(super) discovered: bool,
    pub(super) active: bool,
    pub(super) translation_requested: bool,
    pub(super) font_requested: bool,
    pub(super) translation_active: bool,
    pub(super) font_active: bool,
    pub(super) applied_generation: Option<u64>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(super) struct WorkflowRuntimeView {
    pub(super) workflow_id: Box<str>,
    pub(super) targets: Vec<WorkflowTargetRuntimeView>,
    pub(super) errors: BTreeMap<Box<str>, CommandError>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(super) struct WorkflowActivationView {
    pub(super) workflow_id: Box<str>,
    pub(super) enabled: bool,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(super) struct WorkflowRuntimeDiagnosticsView {
    pub(super) workflow_id: Box<str>,
    pub(super) records: Vec<WorkflowRuntimeTraceView>,
    pub(super) dropped: u64,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(super) struct WorkflowRuntimeTraceView {
    pub(super) software_id: Box<str>,
    pub(super) software_name: Box<str>,
    pub(super) adapter_name: Box<str>,
    pub(super) source_text: Box<str>,
    pub(super) status: Box<str>,
    pub(super) text: Box<str>,
    pub(super) font: Box<str>,
    pub(super) generation: u64,
    pub(super) publication_identity: Box<str>,
    pub(super) translation_digest: Box<str>,
    pub(super) font_policy_digest: Box<str>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(super) struct WorkflowCommandResult {
    pub(super) definition: WorkflowView,
    pub(super) activation: WorkflowActivationView,
    pub(super) runtime: WorkflowRuntimeView,
}

impl DesktopApplication {
    pub(super) fn workflow_detail(&self, workflow_id: &str) -> Result<WorkflowView, CommandError> {
        self.backend.workflow(workflow_id).map_err(|_| {
            CommandError::new("workflow.not_found").with_arg("workflowId", workflow_id)
        })
    }

    pub(super) fn create_workflow(
        &mut self,
        create: WorkflowCreate,
    ) -> Result<DesktopProductSnapshot, CommandError> {
        self.backend
            .create_workflow(create)
            .map_err(|_| CommandError::new("workflow.invalid_create"))?;
        Ok(self.snapshot())
    }

    pub(super) fn update_workflow(
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

    pub(super) fn copy_workflow(
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

    pub(super) fn delete_workflows(
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

    pub(super) fn reconcile_workflow_if_enabled(
        &mut self,
        workflow_id: &str,
    ) -> Result<(), CommandError> {
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

    pub(super) fn reconcile_enabled_workflows(&mut self) -> Result<(), CommandError> {
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

    pub(super) fn restore_enabled_workflows(&mut self) -> Result<(), CommandError> {
        self.reconcile_enabled_workflows()
    }

    pub(super) fn refresh_workflows(&mut self) -> Result<DesktopProductSnapshot, CommandError> {
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

    pub(super) fn control_workflow_diagnostics(
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

    pub(super) fn workflow_diagnostics(
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

    pub(super) fn enable_workflow(
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

    pub(super) fn disable_workflow(
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
}

pub(super) fn runtime_command_error(error: DesktopRuntimeError, enabling: bool) -> CommandError {
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
            | HostOperationFailure::RemoteThreadUnavailable
            | HostOperationFailure::IsolatedWorkerPermissionDenied => {
                CommandError::new("runtime.target_access_failed")
            }
            HostOperationFailure::RuntimeModuleUnavailable => {
                CommandError::new("runtime.component_load_failed")
            }
            HostOperationFailure::TargetRuntimeRestartRequired => {
                CommandError::new("runtime.target_restart_required")
            }
            HostOperationFailure::RuntimeExportUnavailable
            | HostOperationFailure::TargetRuntimeRejected(_) => {
                CommandError::new("runtime.component_incompatible")
            }
            HostOperationFailure::RemoteThreadTimeout
            | HostOperationFailure::IsolatedWorkerTimeout => {
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

pub(super) fn workflow_runtime_view(
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

pub(super) fn idle_workflow_runtime_view(
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
#[tauri::command]
pub(super) fn desktop_workflow(
    workflow_id: String,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<WorkflowView, CommandError> {
    application
        .lock()
        .map_err(|_| workspace_unavailable())?
        .workflow_detail(&workflow_id)
}

#[tauri::command]
pub(super) fn desktop_create_workflow(
    create: WorkflowCreate,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<DesktopProductSnapshot, CommandError> {
    application
        .lock()
        .map_err(|_| workspace_unavailable())?
        .create_workflow(create)
}

#[tauri::command]
pub(super) fn desktop_update_workflow(
    edit: WorkflowEdit,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<DesktopProductSnapshot, CommandError> {
    application
        .lock()
        .map_err(|_| workspace_unavailable())?
        .update_workflow(edit)
}

#[tauri::command]
pub(super) fn desktop_copy_workflow(
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
pub(super) fn desktop_delete_workflows(
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
pub(super) fn desktop_enable_workflow(
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
pub(super) fn desktop_disable_workflow(
    workflow_id: String,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<WorkflowCommandResult, CommandError> {
    application
        .lock()
        .map_err(|_| runtime_unavailable())?
        .disable_workflow(&workflow_id)
}

#[tauri::command]
pub(super) fn desktop_refresh_workflows(
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<DesktopProductSnapshot, CommandError> {
    application
        .lock()
        .map_err(|_| runtime_unavailable())?
        .refresh_workflows()
}

#[tauri::command]
pub(super) fn desktop_control_workflow_diagnostics(
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
pub(super) fn desktop_workflow_diagnostics(
    workflow_id: String,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<WorkflowRuntimeDiagnosticsView, CommandError> {
    application
        .lock()
        .map_err(|_| runtime_unavailable())?
        .workflow_diagnostics(&workflow_id)
}

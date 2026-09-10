use super::*;
fn capture_backend_error(error: BackendError) -> CommandError {
    match error {
        BackendError::SoftwareBindingMissing(id) => CommandError::new("software.binding_missing")
            .with_arg("softwareId", id.to_string()),
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

pub(super) fn probe_run_error(error: ProbeRunError) -> CommandError {
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

pub(super) fn capture_preview_publish_error(_error: DesktopRuntimeError) -> CommandError {
    CommandError::new("capture.preview_publish_failed")
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ProbeRunCreateRequest {
    pub(super) id: Box<str>,
    pub(super) name: Box<str>,
    pub(super) software_id: Box<str>,
    pub(super) adapter_ids: Vec<Box<str>>,
    pub(super) live_preview_enabled: bool,
    #[serde(default)]
    pub(super) excluded_dictionary_ids: Vec<Box<str>>,
    pub(super) dictionary: ProbeDictionaryBindingRequest,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ProbeRunUpdateRequest {
    pub(super) run_id: Box<str>,
    pub(super) name: Box<str>,
    pub(super) dictionary_id: Box<str>,
    #[serde(default)]
    pub(super) excluded_dictionary_ids: Vec<Box<str>>,
    pub(super) adapter_ids: Vec<Box<str>>,
    pub(super) live_preview_enabled: bool,
}

#[derive(Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub(super) enum ProbeDictionaryBindingRequest {
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
pub(super) struct ProbeRunQueryRequest {
    pub(super) run_id: Box<str>,
    pub(super) search: Box<str>,
    #[serde(default)]
    pub(super) adapter_ids: Vec<Box<str>>,
    #[serde(default)]
    pub(super) translation_filter: ProbeTranslationFilter,
    pub(super) page: usize,
    pub(super) page_size: usize,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ProbeTranslationEditRequest {
    pub(super) run_id: Box<str>,
    pub(super) source: Box<str>,
    pub(super) translation: Box<str>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ProbeDictionarySyncEntryRequest {
    pub(super) source: Box<str>,
    pub(super) translation: Box<str>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ProbeDictionarySyncRequest {
    pub(super) run_id: Box<str>,
    pub(super) entries: Vec<ProbeDictionarySyncEntryRequest>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ProbeBulkRequest {
    pub(super) run_id: Box<str>,
    pub(super) sources: Vec<Box<str>>,
    pub(super) action: Box<str>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ProbeExportRequest {
    pub(super) run_id: Box<str>,
    pub(super) format: ProbeExportFormat,
    pub(super) output_path: PathBuf,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(super) struct ProbeRunView {
    pub(super) workflow_runtime: Option<WorkflowRuntimeView>,
    #[serde(flatten)]
    pub(super) summary: ProbeRunSummary,
    pub(super) dictionary_revision: u64,
    pub(super) exclusion_revisions: Vec<u64>,
    pub(super) dictionary_entry_count: usize,
    pub(super) runtime_capability: Option<ProbeRuntimeCapability>,
    pub(super) quick_probe: bool,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum ProbeRuntimeCapability {
    DirectReplace,
    CollectionOnly,
    NoSignal,
}

impl ProbeRuntimeCapability {
    pub(super) fn from_status(status: &DesktopRuntimeStatus) -> Self {
        if status.is_feature_active(Feature::TextReplace) {
            Self::DirectReplace
        } else if status.is_feature_active(Feature::TextObserve) {
            Self::CollectionOnly
        } else {
            Self::NoSignal
        }
    }
}

impl DesktopApplication {
    pub(super) fn compatible_probe_adapter_ids(
        &self,
        software_id: &str,
    ) -> Result<Vec<Box<str>>, CommandError> {
        let snapshot = self.backend.snapshot();
        let software = snapshot
            .software()
            .iter()
            .find(|software| software.id() == software_id)
            .ok_or_else(|| {
                CommandError::new("capture.unknown_software").with_arg("softwareId", software_id)
            })?;
        let executable_path = software
            .executable_path()
            .ok_or_else(|| CommandError::new("software.invalid_executable"))?;
        let executable = inspect_windows_executable(executable_path)
            .map_err(|_| CommandError::new("software.invalid_executable"))?;

        Ok(self
            .adapter_target_support
            .iter()
            .filter(|(_, support)| {
                support.supports("windows", executable.architecture(), Feature::TextObserve)
            })
            .map(|(adapter_id, _)| adapter_id.clone())
            .collect())
    }

    fn ensure_compatible_probe_adapters(
        &self,
        software_id: &str,
        adapter_ids: &[Box<str>],
    ) -> Result<(), CommandError> {
        if let Some(error) = self.runtime_bundle_command_error() {
            return Err(error);
        }
        if adapter_ids.is_empty() {
            return Err(CommandError::new("capture.adapters_required"));
        }
        let compatible = self
            .compatible_probe_adapter_ids(software_id)?
            .into_iter()
            .collect::<BTreeSet<_>>();
        if adapter_ids.iter().all(|id| compatible.contains(id)) {
            Ok(())
        } else {
            Err(CommandError::new("capture.unknown_adapter"))
        }
    }

    pub(super) fn probe_run_view(
        &self,
        summary: ProbeRunSummary,
    ) -> Result<ProbeRunView, CommandError> {
        let dictionary = self
            .backend
            .dictionary(summary.dictionary_id())
            .map_err(|_| {
                CommandError::new("dictionary.not_found")
                    .with_arg("dictionaryId", summary.dictionary_id())
            })?;
        Ok(ProbeRunView {
            workflow_runtime: summary.workflow_id().and_then(|id| self.workflow_runtime_status.get(id))
                .cloned().map(|runtime| self.project_workflow_runtime(runtime)),
            runtime_capability: if let Some(owner) = summary.workflow_id() {
                self.workflow_runtime_status.get(owner)
                    .and_then(|runtime| runtime.targets.iter().find(|target| target.software_id.as_ref() == summary.software_id() && target.active))
                    .map(|target| if target.translation_active { ProbeRuntimeCapability::DirectReplace } else { ProbeRuntimeCapability::CollectionOnly })
            } else {
                (self.active_probe_run_id.as_deref() == Some(summary.id())).then_some(self.active_probe_capability).flatten()
            },
            quick_probe: false,
            exclusion_revisions: summary.excluded_dictionary_ids().iter()
                .map(|id| self.backend.dictionary(id).map(|dictionary| dictionary.revision())
                    .map_err(|_| CommandError::new("dictionary.not_found").with_arg("dictionaryId", id.to_string())))
                .collect::<Result<Vec<_>, _>>()?,
            summary,
            dictionary_revision: dictionary.revision(),
            dictionary_entry_count: dictionary.entries().len(),
        })
    }

    pub(super) fn probe_dictionary_snapshot(
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

    fn validate_probe_exclusions(&self, dictionary_id: &str, excluded: &[Box<str>]) -> Result<(), CommandError> {
        if excluded.iter().any(|id| id.as_ref() == dictionary_id) || excluded.iter().collect::<BTreeSet<_>>().len() != excluded.len() {
            return Err(CommandError::new("capture.invalid_configuration"));
        }
        for id in excluded { self.backend.dictionary(id).map_err(|_| CommandError::new("dictionary.not_found").with_arg("dictionaryId", id.to_string()))?; }
        Ok(())
    }

    pub(super) fn probe_snapshot_key(&self, summary: &ProbeRunSummary) -> Result<Vec<(Box<str>, u64)>, CommandError> {
        std::iter::once(summary.dictionary_id()).chain(summary.excluded_dictionary_ids().iter().map(|id| id.as_ref()))
            .map(|id| self.backend.dictionary(id).map(|dictionary| (id.into(), dictionary.revision()))
                .map_err(|_| CommandError::new("dictionary.not_found").with_arg("dictionaryId", id))).collect()
    }

    pub(super) fn probe_entries_snapshot(&self, summary: &ProbeRunSummary) -> Result<std::sync::Arc<ProbeDictionarySnapshot>, CommandError> {
        let key = self.probe_snapshot_key(summary)?;
        if let Some((previous, snapshot)) = self.probe_snapshot_cache.borrow().as_ref() {
            if previous == &key { return Ok(snapshot.clone()); }
        }
        let mut excluded = BTreeSet::new();
        for id in summary.excluded_dictionary_ids() {
            let dictionary = self.backend.dictionary(id).map_err(|_| CommandError::new("dictionary.not_found").with_arg("dictionaryId", id.to_string()))?;
            excluded.extend(dictionary.entries().iter().map(|entry| entry.source().to_owned()));
        }
        let snapshot = std::sync::Arc::new(self.probe_dictionary_snapshot(summary.dictionary_id())?.with_excluded_sources(excluded));
        *self.probe_snapshot_cache.borrow_mut() = Some((key, snapshot.clone()));
        Ok(snapshot)
    }

    pub(super) fn probe_run_list(&mut self) -> Result<Vec<ProbeRunView>, CommandError> {
        let summaries = self.probe_runs.list().map_err(probe_run_error)?;
        summaries
            .into_iter()
            .map(|summary| self.probe_run_view(summary))
            .collect()
    }

    pub(super) fn create_probe_run(
        &mut self,
        request: ProbeRunCreateRequest,
    ) -> Result<ProbeRunView, CommandError> {
        let held_probe_exists =
            self.probe_runs
                .list()
                .map_err(probe_run_error)?
                .iter()
                .any(|summary| {
                    matches!(
                        summary.status(),
                        ProbeRunStatus::Running | ProbeRunStatus::Paused
                    )
                });
        if self.active_probe_run_id.is_some() || held_probe_exists {
            return Err(CommandError::new("capture.already_active"));
        }
        let adapter_ids = request.adapter_ids.clone();
        self.ensure_compatible_probe_adapters(&request.software_id, &adapter_ids)?;
        if request.live_preview_enabled && !self.adapters_support_preview(&adapter_ids) {
            return Err(CommandError::new("capture.preview_unavailable"));
        }
        let requested_dictionary_id = match &request.dictionary {
            ProbeDictionaryBindingRequest::Existing { dictionary_id } => dictionary_id,
            ProbeDictionaryBindingRequest::New { id, .. } => id,
        };
        self.validate_probe_exclusions(requested_dictionary_id, &request.excluded_dictionary_ids)?;
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
        self.validate_probe_exclusions(&dictionary_id, &request.excluded_dictionary_ids)?;
        let create = ProbeRunCreate::new(
            request.id,
            request.name,
            request.software_id,
            dictionary_id,
            adapter_ids,
            request.live_preview_enabled,
        )
        .map_err(probe_run_error)?;
        let create = create.with_excluded_dictionaries(request.excluded_dictionary_ids).map_err(probe_run_error)?;
        let summary = self.probe_runs.create(create).map_err(probe_run_error)?;
        self.start_probe_run_runtime(summary.id(), true)
    }

    pub(super) fn delete_probe_runs(&mut self, run_ids: &[Box<str>]) -> Result<(), CommandError> {
        if run_ids.is_empty() {
            return Err(CommandError::new("capture.invalid_configuration"));
        }
        let run_ids = run_ids.iter().cloned().collect::<BTreeSet<_>>();
        for run_id in &run_ids {
            let summary = self.probe_runs.summary(run_id).map_err(probe_run_error)?;
            self.ensure_ai_dictionary_writable(summary.dictionary_id())?;
            if summary.workflow_id().is_some() {
                return Err(CommandError::new("capture.invalid_state"));
            }
            if !self.quick_probe_sessions.contains(run_id)
                && (self.active_probe_run_id.as_deref() == Some(run_id)
                    || matches!(
                        summary.status(),
                        ProbeRunStatus::Running | ProbeRunStatus::Paused
                    ))
            {
                return Err(CommandError::new("capture.invalid_state"));
            }
        }
        for run_id in &run_ids {
            if !self.quick_probe_sessions.contains(run_id) {
                self.probe_runs.delete(run_id).map_err(probe_run_error)?;
            }
        }
        for run_id in &run_ids {
            if self.quick_probe_sessions.contains(run_id) {
                self.cleanup_quick_probe(run_id)?;
            }
        }
        Ok(())
    }

    pub(super) fn update_probe_run(
        &mut self,
        request: ProbeRunUpdateRequest,
    ) -> Result<ProbeRunView, CommandError> {
        if self.probe_runs.summary(&request.run_id).map_err(probe_run_error)?.workflow_id().is_some() {
            return Err(CommandError::new("capture.invalid_configuration"));
        }
        let current = self
            .probe_runs
            .summary(&request.run_id)
            .map_err(probe_run_error)?;
        self.backend
            .dictionary(&request.dictionary_id)
            .map_err(|_| {
                CommandError::new("dictionary.not_found")
                    .with_arg("dictionaryId", request.dictionary_id.to_string())
            })?;
        let adapter_ids = request.adapter_ids.clone();
        self.validate_probe_exclusions(&request.dictionary_id, &request.excluded_dictionary_ids)?;
        let configuration_changed = current.excluded_dictionary_ids() != request.excluded_dictionary_ids.as_slice()
            || current.adapter_ids() != adapter_ids.as_slice()
            || current.dictionary_id() != request.dictionary_id.as_ref()
            || current.live_preview_enabled() != request.live_preview_enabled;
        if configuration_changed {
            self.ensure_compatible_probe_adapters(current.software_id(), &adapter_ids)?;
            self.backend
                .capture_runtime_spec(current.software_id(), &adapter_ids)
                .map_err(capture_backend_error)?;
            if request.live_preview_enabled && !self.adapters_support_preview(&adapter_ids) {
                return Err(CommandError::new("capture.preview_unavailable"));
            }
        }
        let update = ProbeRunUpdate::new(
            request.name,
            request.dictionary_id,
            adapter_ids,
            request.live_preview_enabled,
        )
        .map_err(probe_run_error)?;
        let update = update.with_excluded_dictionaries(request.excluded_dictionary_ids).map_err(probe_run_error)?;
        let summary = self
            .probe_runs
            .update(&request.run_id, update)
            .map_err(probe_run_error)?;
        self.probe_run_view(summary)
    }

    pub(super) fn clear_probe_run_entries(
        &mut self,
        run_id: &str,
    ) -> Result<ProbeRunView, CommandError> {
        let summary = self.probe_runs.summary(run_id).map_err(probe_run_error)?;
        self.ensure_ai_dictionary_writable(summary.dictionary_id())?;
        let is_active = self.active_probe_run_id.as_deref() == Some(run_id);
        if summary.status() == ProbeRunStatus::Running
            || (is_active && summary.status() != ProbeRunStatus::Paused)
        {
            return Err(CommandError::new("capture.invalid_state"));
        }
        let preserve_paused = summary.status() == ProbeRunStatus::Paused;
        if is_active {
            self.runtimes
                .as_mut()
                .ok_or_else(|| CommandError::new("runtime.unavailable"))?
                .stop_capture(summary.software_id())
                .map_err(|error| runtime_command_error(error, false))?;
            self.active_probe_run_id = None;
            self.active_probe_capability = None;
        }
        if preserve_paused {
            self.probe_runs
                .set_status(run_id, ProbeRunStatus::Ready)
                .map_err(probe_run_error)?;
        }
        self.probe_runs
            .clear_observations(run_id)
            .map_err(probe_run_error)?;
        let dictionary = self
            .backend
            .dictionary(summary.dictionary_id())
            .cloned()
            .map_err(|_| CommandError::new("dictionary.not_found"))?;
        let sources = dictionary
            .entries()
            .iter()
            .map(|entry| Box::<str>::from(entry.source()))
            .collect::<Vec<_>>();
        if !sources.is_empty() {
            self.backend
                .delete_dictionary_entries(dictionary.id(), sources, dictionary.revision())
                .map_err(|_| CommandError::new("dictionary.invalid_update"))?;
            self.reconcile_enabled_workflows()?;
        }
        if preserve_paused {
            self.probe_runs
                .set_status(run_id, ProbeRunStatus::Paused)
                .map_err(probe_run_error)?;
        }
        self.probe_run_summary(run_id)
    }

    pub(super) fn resume_probe_run(&mut self, run_id: &str) -> Result<ProbeRunView, CommandError> {
        if let Some(owner) = self.probe_runs.summary(run_id).map_err(probe_run_error)?.workflow_id().map(str::to_owned) {
            self.enable_workflow(&owner, false)?;
            return self.probe_run_summary(run_id);
        }
        self.start_probe_run_runtime(run_id, false)
    }

    pub(super) fn start_probe_run_runtime(
        &mut self,
        run_id: &str,
        allow_offline_target: bool,
    ) -> Result<ProbeRunView, CommandError> {
        if self.active_probe_run_id.is_some() {
            return Err(CommandError::new("capture.already_active"));
        }
        let summary = self.probe_runs.summary(run_id).map_err(probe_run_error)?;
        let another_probe_is_held =
            self.probe_runs
                .list()
                .map_err(probe_run_error)?
                .iter()
                .any(|candidate| {
                    candidate.id() != run_id
                        && matches!(
                            candidate.status(),
                            ProbeRunStatus::Running | ProbeRunStatus::Paused
                        )
                });
        if another_probe_is_held {
            return Err(CommandError::new("capture.already_active"));
        }
        if summary.status() == ProbeRunStatus::Running {
            return Err(CommandError::new("capture.already_active"));
        }
        self.ensure_compatible_probe_adapters(summary.software_id(), summary.adapter_ids())?;
        let spec = self
            .backend
            .capture_runtime_spec(summary.software_id(), summary.adapter_ids())
            .map_err(capture_backend_error)?;
        let configuration = self
            .probe_runs
            .capture_configuration(run_id, DEFAULT_MAX_ENTRIES)
            .map_err(probe_run_error)?;
        let start_result = self
            .runtimes
            .as_mut()
            .ok_or_else(|| CommandError::new("runtime.unavailable"))?
            .start_capture(summary.software_id(), &spec, configuration);
        let runtime_capability = match start_result {
            Ok(capability) => capability,
            Err(DesktopRuntimeError::UnknownTarget) if allow_offline_target => {
                return self.probe_run_view(summary);
            }
            Err(DesktopRuntimeError::TargetInUse(TargetExecutionOwner::Workflow)) => {
                return Err(CommandError::new("capture.target_in_use_by_workflow"));
            }
            Err(error) => return Err(runtime_command_error(error, true)),
        };
        let previous_status = summary.status();
        // Quick probes intentionally hide adapter and preview settings. Enable
        // writeback only after the active Runtime proves that it can replace text.
        let enable_automatic_preview = self.quick_probe_sessions.contains(run_id)
            && runtime_capability == ProbeRuntimeCapability::DirectReplace
            && !summary.live_preview_enabled();
        let summary = if enable_automatic_preview {
            let preview_update = (|| {
                if previous_status == ProbeRunStatus::Paused {
                    self.probe_runs
                        .set_status(run_id, ProbeRunStatus::Ready)
                        .map_err(probe_run_error)?;
                }
                let update = ProbeRunUpdate::new(
                    summary.name(),
                    summary.dictionary_id(),
                    summary.adapter_ids().iter().cloned(),
                    true,
                )
                .map_err(probe_run_error)?;
                let update = update.with_excluded_dictionaries(summary.excluded_dictionary_ids().to_vec()).map_err(probe_run_error)?;
                self.probe_runs
                    .update(run_id, update)
                    .map_err(probe_run_error)
            })();
            match preview_update {
                Ok(summary) => summary,
                Err(error) => {
                    if let Some(runtimes) = self.runtimes.as_mut() {
                        let _ = runtimes.stop_capture(summary.software_id());
                    }
                    if previous_status == ProbeRunStatus::Paused {
                        let _ = self.probe_runs.set_status(run_id, ProbeRunStatus::Paused);
                    }
                    return Err(error);
                }
            }
        } else {
            summary
        };
        self.active_probe_run_id = Some(run_id.into());
        self.active_probe_capability = Some(runtime_capability);
        self.probe_runs
            .set_status(run_id, ProbeRunStatus::Running)
            .map_err(probe_run_error)?;
        if let Err(error) = self.publish_probe_preview_if_active(run_id) {
            if let Some(runtimes) = self.runtimes.as_mut() {
                let _ = runtimes.stop_capture(summary.software_id());
            }
            let _ = self.probe_runs.set_status(run_id, ProbeRunStatus::Ready);
            self.active_probe_run_id = None;
            self.active_probe_capability = None;
            return Err(error);
        }
        let summary = self.probe_runs.summary(run_id).map_err(probe_run_error)?;
        self.probe_run_view(summary)
    }

    pub(super) fn set_probe_run_paused(
        &mut self,
        run_id: &str,
        paused: bool,
    ) -> Result<ProbeRunView, CommandError> {
        let summary = self.probe_runs.summary(run_id).map_err(probe_run_error)?;
        if let Some(owner) = summary.workflow_id() {
            self.backend.set_workflow_collection_enabled(owner, !paused).map_err(|_| CommandError::new("workflow.invalid_update"))?;
            self.update_workflow_collection_status(owner, true)?;
            return self.probe_run_summary(run_id);
        }
        if self.active_probe_run_id.as_deref() != Some(run_id) {
            let summary = self.probe_runs.summary(run_id).map_err(probe_run_error)?;
            if !paused && summary.status() == ProbeRunStatus::Paused {
                return self.start_probe_run_runtime(run_id, false);
            }
            return Err(CommandError::new("capture.not_active"));
        }
        let summary = self.probe_runs.summary(run_id).map_err(probe_run_error)?;
        let control_result = self
            .runtimes
            .as_mut()
            .ok_or_else(|| CommandError::new("runtime.unavailable"))?
            .control_capture(summary.software_id(), paused);
        if control_result.is_err() {
            if paused {
                return self
                    .probe_runs
                    .set_status(run_id, ProbeRunStatus::Paused)
                    .map_err(probe_run_error)
                    .and_then(|summary| self.probe_run_view(summary));
            }
            self.runtimes
                .as_mut()
                .ok_or_else(|| CommandError::new("runtime.unavailable"))?
                .abandon_capture(summary.software_id());
            self.active_probe_run_id = None;
            self.active_probe_capability = None;
            return self.start_probe_run_runtime(run_id, false);
        }
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

    pub(super) fn disconnect_probe_run(
        &mut self,
        run_id: &str,
    ) -> Result<ProbeRunView, CommandError> {
        if let Some(owner) = self.probe_runs.summary(run_id).map_err(probe_run_error)?.workflow_id().map(str::to_owned) {
            self.disable_workflow(&owner)?;
            return self.probe_run_summary(run_id);
        }
        if self.active_probe_run_id.as_deref() != Some(run_id) {
            let summary = self.probe_runs.summary(run_id).map_err(probe_run_error)?;
            if summary.status() == ProbeRunStatus::Paused {
                let summary = self
                    .probe_runs
                    .set_status(run_id, ProbeRunStatus::Ready)
                    .map_err(probe_run_error)?;
                return self.probe_run_view(summary);
            }
            return Err(CommandError::new("capture.not_active"));
        }
        let summary = self.probe_runs.summary(run_id).map_err(probe_run_error)?;
        self.runtimes
            .as_mut()
            .ok_or_else(|| CommandError::new("runtime.unavailable"))?
            .stop_capture(summary.software_id())
            .map_err(|error| runtime_command_error(error, false))?;
        self.active_probe_run_id = None;
        self.active_probe_capability = None;
        let summary = self
            .probe_runs
            .set_status(run_id, ProbeRunStatus::Ready)
            .map_err(probe_run_error)?;
        self.probe_run_view(summary)
    }

    pub(super) fn probe_run_summary(&mut self, run_id: &str) -> Result<ProbeRunView, CommandError> {
        let summary = self.probe_runs.summary(run_id).map_err(probe_run_error)?;
        self.probe_run_view(summary)
    }

    pub(super) fn probe_run_entries(
        &mut self,
        request: ProbeRunQueryRequest,
    ) -> Result<ProbeEntryPage, CommandError> {
        self.probe_run_entries_visible(request, |_| true)
    }

    fn probe_run_entries_visible(&mut self, request: ProbeRunQueryRequest, visible: impl FnMut(&str) -> bool) -> Result<ProbeEntryPage, CommandError> {
        let query = ProbeQuery::new(request.search, request.page, request.page_size)
            .and_then(|query| query.with_adapter_ids(request.adapter_ids))
            .map(|query| query.with_translation_filter(request.translation_filter))
            .map_err(probe_run_error)?;
        let summary = self
            .probe_runs
            .summary(&request.run_id)
            .map_err(probe_run_error)?;
        let dictionary = self.probe_entries_snapshot(&summary)?;
        self.probe_runs
            .query_entries_visible(&request.run_id, &query, &dictionary, visible)
            .map_err(probe_run_error)
    }

    pub(super) fn edit_probe_translation(
        &mut self,
        request: ProbeTranslationEditRequest,
    ) -> Result<ProbeRunView, CommandError> {
        let summary = self
            .probe_runs
            .summary(&request.run_id)
            .map_err(probe_run_error)?;
        self.ensure_ai_dictionary_writable(summary.dictionary_id())?;
        let dictionary = self
            .backend
            .dictionary(summary.dictionary_id())
            .cloned()
            .map_err(|_| CommandError::new("dictionary.not_found"))?;
        let existing = dictionary
            .entries()
            .iter()
            .find(|entry| entry.source() == request.source.as_ref());
        if existing.is_none() && !self.probe_runs.excluded_sources_for(
            &request.run_id, self.probe_entries_snapshot(&summary)?.as_ref(), &[request.source.clone()],
        ).map_err(probe_run_error)?.is_empty() {
            return Err(CommandError::new("capture.source_owned_by_dictionary"));
        }
        let translation = request.translation.trim();
        let changed = if translation.is_empty() {
            let snapshot = self.probe_dictionary_snapshot(dictionary.id())?;
            let mut sources = self.probe_runs.dictionary_sources_for_rows(&request.run_id, &[request.source.clone()], &snapshot).map_err(probe_run_error)?;
            if existing.is_some() && !sources.contains(&request.source) { sources.push(request.source.clone()); }
            if !sources.is_empty() {
                self.backend
                    .delete_dictionary_entries(
                        dictionary.id(),
                        sources,
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

    pub(super) fn sync_probe_dictionary_entries(
        &mut self,
        request: ProbeDictionarySyncRequest,
    ) -> Result<ProbeRunView, CommandError> {
        if request.entries.is_empty() {
            return Err(CommandError::new("capture.invalid_configuration"));
        }
        let summary = self
            .probe_runs
            .summary(&request.run_id)
            .map_err(probe_run_error)?;
        self.ensure_ai_dictionary_writable(summary.dictionary_id())?;
        let dictionary = self
            .backend
            .dictionary(summary.dictionary_id())
            .cloned()
            .map_err(|_| CommandError::new("dictionary.not_found"))?;
        let requested_sources = request.entries.iter().map(|entry| Box::<str>::from(entry.source.trim())).collect::<Vec<_>>();
        let excluded = self.probe_runs.excluded_sources_for(
            &request.run_id, self.probe_entries_snapshot(&summary)?.as_ref(), &requested_sources,
        ).map_err(probe_run_error)?;
        if excluded.iter().any(|source| !dictionary.entries().iter().any(|entry| entry.source() == source.as_ref())) {
            return Err(CommandError::new("capture.source_owned_by_dictionary"));
        }
        let mut sources = BTreeSet::new();
        let mut entries = Vec::with_capacity(request.entries.len());
        for entry in request.entries {
            let source = entry.source.trim();
            let translation = entry.translation.trim();
            if source.is_empty()
                || translation.is_empty()
                || !sources.insert(Box::<str>::from(source))
            {
                return Err(CommandError::new("capture.invalid_configuration"));
            }
            entries.push(DictionaryEntryCreate::new(source, translation));
        }
        let updated_dictionary = self
            .backend
            .upsert_dictionary_entries(dictionary.id(), entries, dictionary.revision())
            .map_err(|_| CommandError::new("dictionary.invalid_update"))?;
        if updated_dictionary.revision() != dictionary.revision() {
            self.reconcile_enabled_workflows()?;
            self.publish_probe_preview_if_active(&request.run_id)?;
        }
        self.probe_run_summary(&request.run_id)
    }

    pub(super) fn bulk_probe_entries(
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
                self.ensure_ai_dictionary_writable(summary.dictionary_id())?;
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
                let snapshot = self.probe_dictionary_snapshot(dictionary.id())?;
                let mut sources = self.probe_runs.dictionary_sources_for_rows(&request.run_id, &request.sources, &snapshot).map_err(probe_run_error)?;
                sources.extend(request
                    .sources
                    .iter()
                    .filter(|source| existing.contains(source.as_ref()))
                    .cloned()
                    .collect::<Vec<_>>());
                sources.sort(); sources.dedup();
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

    pub(super) fn export_probe_run(
        &mut self,
        request: ProbeExportRequest,
    ) -> Result<(), CommandError> {
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
            let dictionary = self.probe_entries_snapshot(&summary)?;
            self.probe_runs
                .export(&request.run_id, request.format, &dictionary)
                .map_err(probe_run_error)?
        };
        std::fs::write(request.output_path, content)
            .map_err(|_| CommandError::new("capture.export_failed"))
    }

    pub(super) fn adapters_support_preview(&self, adapter_ids: &[Box<str>]) -> bool {
        !self.preview_adapter_ids(adapter_ids).is_empty()
    }

    fn adapter_supports_replacement(&self, adapter_id: &str) -> bool {
        self.adapters.iter().any(|adapter| {
            adapter.id.as_ref() == adapter_id
                && adapter
                    .features
                    .iter()
                    .any(|feature| feature.as_ref() == "textReplace")
        })
    }

    pub(super) fn preview_adapter_ids(&self, adapter_ids: &[Box<str>]) -> Vec<Box<str>> {
        adapter_ids
            .iter()
            .filter(|adapter_id| self.adapter_supports_replacement(adapter_id))
            .cloned()
            .collect()
    }

    pub(super) fn refresh_probe_text(
        &mut self,
        run_id: &str,
    ) -> Result<ProbeRunView, CommandError> {
        let summary = self.probe_runs.summary(run_id).map_err(probe_run_error)?;
        if let Some(owner) = summary.workflow_id() {
            if !self.workflow_runtime_status.get(owner).is_some_and(|runtime| runtime.targets.iter().any(|target|
                target.software_id.as_ref() == summary.software_id() && target.active && target.translation_active)) {
                return Err(CommandError::new("capture.not_active"));
            }
            let intent = self.backend.effective_workflow_intent(owner).map_err(|_| CommandError::new("workflow.invalid"))?;
            let target = intent.targets().iter().find(|target| target.software_id() == summary.software_id()).ok_or_else(|| CommandError::new("workflow.invalid"))?;
            self.runtimes.as_mut().ok_or_else(runtime_unavailable)?.publish_capture(summary.software_id(), target.runtime_spec().publication().clone())
                .map_err(|error| runtime_command_error(error, true))?;
            return self.probe_run_summary(run_id);
        }
        if self.active_probe_run_id.as_deref() != Some(run_id)
            || !matches!(
                summary.status(),
                ProbeRunStatus::Running | ProbeRunStatus::Paused
            )
        {
            return Err(CommandError::new("capture.not_active"));
        }
        if !summary.live_preview_enabled() {
            return Err(CommandError::new("capture.preview_unavailable"));
        }
        // Republish the current dictionary to dispatch native refresh callbacks
        // and asynchronous redraw. Acceptance does not prove visual completion.
        self.publish_probe_preview_if_active(run_id)?;
        let summary = self.probe_runs.summary(run_id).map_err(probe_run_error)?;
        self.probe_run_view(summary)
    }

    pub(super) fn publish_probe_preview_if_active(
        &mut self,
        run_id: &str,
    ) -> Result<(), CommandError> {
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
        let dictionary = self.probe_entries_snapshot(&summary)?;
        let entries = self
            .probe_runs
            .preview_entries(run_id, &dictionary)
            .map_err(probe_run_error)?;
        let preview_adapter_ids = self.preview_adapter_ids(summary.adapter_ids());
        if preview_adapter_ids.is_empty() {
            return Err(CommandError::new("capture.preview_unavailable"));
        }
        let mut snapshot = TranslationSnapshot::empty(Generation::new(generation));
        for location in locations {
            for entry in &entries {
                snapshot = snapshot.with_entry_for_adapters(
                    location.clone(),
                    entry.source(),
                    entry.translation(),
                    preview_adapter_ids.iter().cloned(),
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
            .map_err(capture_preview_publish_error)?;
        self.probe_runs
            .set_preview_generation(run_id, generation)
            .map_err(probe_run_error)?;
        Ok(())
    }
}

#[tauri::command]
pub(super) fn desktop_probe_runs(
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<Vec<ProbeRunView>, CommandError> {
    application
        .lock()
        .map_err(|_| workspace_unavailable())?
        .probe_run_list()
}

#[tauri::command]
pub(super) fn desktop_compatible_probe_adapters(
    software_id: String,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<Vec<Box<str>>, CommandError> {
    application
        .lock()
        .map_err(|_| workspace_unavailable())?
        .compatible_probe_adapter_ids(&software_id)
}

#[tauri::command]
pub(super) fn desktop_create_probe_run(
    request: ProbeRunCreateRequest,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<ProbeRunView, CommandError> {
    application
        .lock()
        .map_err(|_| runtime_unavailable())?
        .create_probe_run(request)
}

#[tauri::command]
pub(super) fn desktop_delete_probe_runs(
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
pub(super) fn desktop_update_probe_run(
    request: ProbeRunUpdateRequest,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<ProbeRunView, CommandError> {
    application
        .lock()
        .map_err(|_| workspace_unavailable())?
        .update_probe_run(request)
}

#[tauri::command]
pub(super) fn desktop_clear_probe_run_entries(
    run_id: String,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<ProbeRunView, CommandError> {
    application
        .lock()
        .map_err(|_| workspace_unavailable())?
        .clear_probe_run_entries(&run_id)
}

#[tauri::command]
pub(super) fn desktop_resume_probe_run(
    run_id: String,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<ProbeRunView, CommandError> {
    application
        .lock()
        .map_err(|_| runtime_unavailable())?
        .resume_probe_run(&run_id)
}

#[tauri::command]
pub(super) fn desktop_set_probe_run_paused(
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
pub(super) fn desktop_refresh_probe_text(
    run_id: String,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<ProbeRunView, CommandError> {
    application
        .lock()
        .map_err(|_| runtime_unavailable())?
        .refresh_probe_text(&run_id)
}

#[tauri::command]
pub(super) fn desktop_disconnect_probe_run(
    run_id: String,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<ProbeRunView, CommandError> {
    application
        .lock()
        .map_err(|_| runtime_unavailable())?
        .disconnect_probe_run(&run_id)
}

#[tauri::command]
pub(super) fn desktop_probe_run_summary(
    run_id: String,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<ProbeRunView, CommandError> {
    application
        .lock()
        .map_err(|_| workspace_unavailable())?
        .probe_run_summary(&run_id)
}

#[tauri::command]
pub(super) fn desktop_probe_run_entries(
    request: ProbeRunQueryRequest,
    hide_skipped: Option<bool>,
    application: State<'_, Mutex<DesktopApplication>>,
    settings: State<'_, Mutex<AppSettingsStore>>,
) -> Result<ProbeEntryPage, CommandError> {
    if !hide_skipped.unwrap_or(false) {
        return application.lock().map_err(|_| workspace_unavailable())?.probe_run_entries(request);
    }
    let policy = settings.lock().map_err(|_| workspace_unavailable())?.current()
        .map_err(|_| workspace_unavailable())?.text_filter_policy().clone();
    let mut cache = crate::ai::source_filter_cache().lock().map_err(|_| workspace_unavailable())?;
    cache.configure(&policy).map_err(crate::ai::ai_plan_error)?;
    application.lock().map_err(|_| workspace_unavailable())?
        .probe_run_entries_visible(request, |source| !cache.hidden(source))
}

#[tauri::command]
pub(super) fn desktop_edit_probe_translation(
    request: ProbeTranslationEditRequest,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<ProbeRunView, CommandError> {
    application
        .lock()
        .map_err(|_| runtime_unavailable())?
        .edit_probe_translation(request)
}

#[tauri::command]
pub(super) fn desktop_sync_probe_dictionary_entries(
    request: ProbeDictionarySyncRequest,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<ProbeRunView, CommandError> {
    application
        .lock()
        .map_err(|_| runtime_unavailable())?
        .sync_probe_dictionary_entries(request)
}

#[tauri::command]
pub(super) fn desktop_bulk_probe_entries(
    request: ProbeBulkRequest,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<ProbeRunView, CommandError> {
    application
        .lock()
        .map_err(|_| runtime_unavailable())?
        .bulk_probe_entries(request)
}

#[tauri::command]
pub(super) fn desktop_export_probe_run(
    request: ProbeExportRequest,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<(), CommandError> {
    application
        .lock()
        .map_err(|_| workspace_unavailable())?
        .export_probe_run(request)
}

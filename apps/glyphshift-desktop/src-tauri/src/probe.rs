use super::*;
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
    pub(super) dictionary: ProbeDictionaryBindingRequest,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ProbeRunUpdateRequest {
    pub(super) run_id: Box<str>,
    pub(super) name: Box<str>,
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
    #[serde(flatten)]
    pub(super) summary: ProbeRunSummary,
    pub(super) dictionary_revision: u64,
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
            runtime_capability: (self.active_probe_run_id.as_deref() == Some(summary.id()))
                .then_some(self.active_probe_capability)
                .flatten(),
            quick_probe: self.quick_probe_sessions.contains(summary.id()),
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
        if self.active_probe_run_id.is_some() {
            return Err(CommandError::new("capture.already_active"));
        }
        self.ensure_compatible_probe_adapters(&request.software_id, &request.adapter_ids)?;
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
        self.start_probe_run_runtime(summary.id(), true)
    }

    pub(super) fn delete_probe_runs(&mut self, run_ids: &[Box<str>]) -> Result<(), CommandError> {
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

    pub(super) fn update_probe_run(
        &mut self,
        request: ProbeRunUpdateRequest,
    ) -> Result<ProbeRunView, CommandError> {
        let current = self
            .probe_runs
            .summary(&request.run_id)
            .map_err(probe_run_error)?;
        let configuration_changed = current.adapter_ids() != request.adapter_ids.as_slice()
            || current.live_preview_enabled() != request.live_preview_enabled;
        if configuration_changed {
            self.ensure_compatible_probe_adapters(current.software_id(), &request.adapter_ids)?;
            self.backend
                .capture_runtime_spec(current.software_id(), &request.adapter_ids)
                .map_err(capture_backend_error)?;
            if request.live_preview_enabled && !self.adapters_support_preview(&request.adapter_ids)
            {
                return Err(CommandError::new("capture.preview_unavailable"));
            }
        }
        let update = ProbeRunUpdate::new(
            request.name,
            request.adapter_ids,
            request.live_preview_enabled,
        )
        .map_err(probe_run_error)?;
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
        if self.active_probe_run_id.as_deref() == Some(run_id) {
            return Err(CommandError::new("capture.invalid_state"));
        }
        let summary = self.probe_runs.summary(run_id).map_err(probe_run_error)?;
        if matches!(
            summary.status(),
            ProbeRunStatus::Running | ProbeRunStatus::Paused
        ) {
            return Err(CommandError::new("capture.invalid_state"));
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
        self.probe_run_summary(run_id)
    }

    pub(super) fn resume_probe_run(&mut self, run_id: &str) -> Result<ProbeRunView, CommandError> {
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
        if matches!(
            summary.status(),
            ProbeRunStatus::Running | ProbeRunStatus::Paused
        ) {
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
            Err(error) => return Err(runtime_command_error(error, true)),
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

    pub(super) fn disconnect_probe_run(
        &mut self,
        run_id: &str,
    ) -> Result<ProbeRunView, CommandError> {
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

    pub(super) fn edit_probe_translation(
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
            let dictionary = self.probe_dictionary_snapshot(summary.dictionary_id())?;
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

    pub(super) fn preview_adapter_ids(&self, adapter_ids: &[Box<str>]) -> Vec<Box<str>> {
        adapter_ids
            .iter()
            .filter(|adapter_id| {
                self.adapters.iter().any(|adapter| {
                    adapter.id.as_ref() == adapter_id.as_ref()
                        && adapter
                            .features
                            .iter()
                            .any(|feature| feature.as_ref() == "textReplace")
                })
            })
            .cloned()
            .collect()
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
        let dictionary = self.probe_dictionary_snapshot(summary.dictionary_id())?;
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
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<ProbeEntryPage, CommandError> {
    application
        .lock()
        .map_err(|_| workspace_unavailable())?
        .probe_run_entries(request)
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

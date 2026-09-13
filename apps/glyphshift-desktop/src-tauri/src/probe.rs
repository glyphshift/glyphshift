use super::*;

pub(super) mod commands;
mod entries;
mod runtime;
mod types;

pub(super) use types::*;

fn capture_backend_error(error: BackendError) -> CommandError {
    match error {
        BackendError::SoftwareBindingMissing(id) => {
            CommandError::new("software.binding_missing").with_arg("softwareId", id.to_string())
        }
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
            workflow_runtime: summary
                .workflow_id()
                .and_then(|id| self.workflow_runtime_status.get(id))
                .cloned()
                .map(|runtime| self.project_workflow_runtime(runtime)),
            runtime_capability: if let Some(owner) = summary.workflow_id() {
                self.workflow_runtime_status
                    .get(owner)
                    .and_then(|runtime| {
                        runtime.targets.iter().find(|target| {
                            target.software_id.as_ref() == summary.software_id() && target.active
                        })
                    })
                    .map(|target| {
                        if target.translation_active {
                            ProbeRuntimeCapability::DirectReplace
                        } else {
                            ProbeRuntimeCapability::CollectionOnly
                        }
                    })
            } else {
                (self.active_probe_run_id.as_deref() == Some(summary.id()))
                    .then_some(self.active_probe_capability)
                    .flatten()
            },
            quick_probe: false,
            exclusion_revisions: summary
                .excluded_dictionary_ids()
                .iter()
                .map(|id| {
                    self.backend
                        .dictionary(id)
                        .map(|dictionary| dictionary.revision())
                        .map_err(|_| {
                            CommandError::new("dictionary.not_found")
                                .with_arg("dictionaryId", id.to_string())
                        })
                })
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

    fn validate_probe_exclusions(
        &self,
        dictionary_id: &str,
        excluded: &[Box<str>],
    ) -> Result<(), CommandError> {
        if excluded.iter().any(|id| id.as_ref() == dictionary_id)
            || excluded.iter().collect::<BTreeSet<_>>().len() != excluded.len()
        {
            return Err(CommandError::new("capture.invalid_configuration"));
        }
        for id in excluded {
            self.backend.dictionary(id).map_err(|_| {
                CommandError::new("dictionary.not_found").with_arg("dictionaryId", id.to_string())
            })?;
        }
        Ok(())
    }

    pub(super) fn probe_snapshot_key(
        &self,
        summary: &ProbeRunSummary,
    ) -> Result<Vec<(Box<str>, u64)>, CommandError> {
        std::iter::once(summary.dictionary_id())
            .chain(
                summary
                    .excluded_dictionary_ids()
                    .iter()
                    .map(|id| id.as_ref()),
            )
            .map(|id| {
                self.backend
                    .dictionary(id)
                    .map(|dictionary| (id.into(), dictionary.revision()))
                    .map_err(|_| {
                        CommandError::new("dictionary.not_found").with_arg("dictionaryId", id)
                    })
            })
            .collect()
    }

    pub(super) fn probe_entries_snapshot(
        &self,
        summary: &ProbeRunSummary,
    ) -> Result<std::sync::Arc<ProbeDictionarySnapshot>, CommandError> {
        let key = self.probe_snapshot_key(summary)?;
        if let Some((previous, snapshot)) = self.probe_snapshot_cache.borrow().as_ref() {
            if previous == &key {
                return Ok(snapshot.clone());
            }
        }
        let mut excluded = BTreeSet::new();
        for id in summary.excluded_dictionary_ids() {
            let dictionary = self.backend.dictionary(id).map_err(|_| {
                CommandError::new("dictionary.not_found").with_arg("dictionaryId", id.to_string())
            })?;
            excluded.extend(
                dictionary
                    .entries()
                    .iter()
                    .map(|entry| entry.source().to_owned()),
            );
        }
        let snapshot = std::sync::Arc::new(
            self.probe_dictionary_snapshot(summary.dictionary_id())?
                .with_excluded_sources(excluded),
        );
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
        let create = create
            .with_excluded_dictionaries(request.excluded_dictionary_ids)
            .map_err(probe_run_error)?;
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
        if self
            .probe_runs
            .summary(&request.run_id)
            .map_err(probe_run_error)?
            .workflow_id()
            .is_some()
        {
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
        let configuration_changed = current.excluded_dictionary_ids()
            != request.excluded_dictionary_ids.as_slice()
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
        let update = update
            .with_excluded_dictionaries(request.excluded_dictionary_ids)
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
}

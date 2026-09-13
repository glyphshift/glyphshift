use super::*;

impl DesktopApplication {
    pub(crate) fn resume_probe_run(&mut self, run_id: &str) -> Result<ProbeRunView, CommandError> {
        if let Some(owner) = self
            .probe_runs
            .summary(run_id)
            .map_err(probe_run_error)?
            .workflow_id()
            .map(str::to_owned)
        {
            self.enable_workflow(&owner, false)?;
            return self.probe_run_summary(run_id);
        }
        self.start_probe_run_runtime(run_id, false)
    }

    pub(crate) fn start_probe_run_runtime(
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
                let update = update
                    .with_excluded_dictionaries(summary.excluded_dictionary_ids().to_vec())
                    .map_err(probe_run_error)?;
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

    pub(crate) fn set_probe_run_paused(
        &mut self,
        run_id: &str,
        paused: bool,
    ) -> Result<ProbeRunView, CommandError> {
        let summary = self.probe_runs.summary(run_id).map_err(probe_run_error)?;
        if let Some(owner) = summary.workflow_id() {
            self.backend
                .set_workflow_collection_enabled(owner, !paused)
                .map_err(|_| CommandError::new("workflow.invalid_update"))?;
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

    pub(crate) fn disconnect_probe_run(
        &mut self,
        run_id: &str,
    ) -> Result<ProbeRunView, CommandError> {
        if let Some(owner) = self
            .probe_runs
            .summary(run_id)
            .map_err(probe_run_error)?
            .workflow_id()
            .map(str::to_owned)
        {
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

    pub(crate) fn adapters_support_preview(&self, adapter_ids: &[Box<str>]) -> bool {
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

    pub(crate) fn preview_adapter_ids(&self, adapter_ids: &[Box<str>]) -> Vec<Box<str>> {
        adapter_ids
            .iter()
            .filter(|adapter_id| self.adapter_supports_replacement(adapter_id))
            .cloned()
            .collect()
    }

    pub(crate) fn refresh_probe_text(
        &mut self,
        run_id: &str,
    ) -> Result<ProbeRunView, CommandError> {
        let summary = self.probe_runs.summary(run_id).map_err(probe_run_error)?;
        if let Some(owner) = summary.workflow_id() {
            if !self
                .workflow_runtime_status
                .get(owner)
                .is_some_and(|runtime| {
                    runtime.targets.iter().any(|target| {
                        target.software_id.as_ref() == summary.software_id()
                            && target.active
                            && target.translation_active
                    })
                })
            {
                return Err(CommandError::new("capture.not_active"));
            }
            let intent = self
                .backend
                .effective_workflow_intent(owner)
                .map_err(|_| CommandError::new("workflow.invalid"))?;
            let target = intent
                .targets()
                .iter()
                .find(|target| target.software_id() == summary.software_id())
                .ok_or_else(|| CommandError::new("workflow.invalid"))?;
            self.runtimes
                .as_mut()
                .ok_or_else(runtime_unavailable)?
                .publish_capture(
                    summary.software_id(),
                    target.runtime_spec().publication().clone(),
                )
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

    pub(crate) fn publish_probe_preview_if_active(
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

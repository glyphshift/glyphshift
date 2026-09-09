use super::*;
use crate::probe::probe_run_error;

impl DesktopApplication {
    fn collection_record_id(&mut self, workflow_id: &str, software_id: &str, dictionary_id: &str, index: usize) -> Result<String, CommandError> {
        let records = self.probe_runs.list().map_err(probe_run_error)?;
        let matches = records.iter().filter(|run| run.workflow_id() == Some(workflow_id)
            && run.software_id() == software_id && run.dictionary_id() == dictionary_id).collect::<Vec<_>>();
        match matches.as_slice() {
            [] => {
                let base = format!("collection-{workflow_id}-{index}");
                let mut candidate = base.clone();
                let mut suffix = 1;
                while records.iter().any(|run| run.id() == candidate) {
                    candidate = format!("{base}-{suffix}");
                    suffix += 1;
                }
                Ok(candidate)
            }
            [run] => Ok(run.id().to_owned()),
            _ => Err(CommandError::new("capture.invalid_configuration")),
        }
    }

    pub(super) fn collect_workflow_sources(&mut self, run_id: &str) -> Result<(), CommandError> {
        let summary = self.probe_runs.summary(run_id).map_err(probe_run_error)?;
        let Some(workflow_id) = summary.workflow_id() else { return Ok(()) };
        let workflow = self.backend.workflow(workflow_id).map_err(|_| CommandError::new("workflow.invalid"))?;
        if !workflow.targets().iter().any(|target| target.software_id() == summary.software_id()
            && target.write_dictionary_id() == Some(summary.dictionary_id())) { return Ok(()) }
        if self.ensure_ai_dictionary_writable(summary.dictionary_id()).is_err() { return Ok(()) }
        let snapshot = self.probe_entries_snapshot(&summary)?;
        let sources = self.probe_runs.uncollected_sources(run_id, &snapshot).map_err(probe_run_error)?;
        if !sources.is_empty() {
            let dictionary = self.backend.dictionary(summary.dictionary_id()).cloned().map_err(|_| CommandError::new("dictionary.not_found"))?;
            let entries = dictionary.entries().iter().map(|entry| DictionaryEntryCreate::new(entry.source(), entry.translation()))
                .chain(sources.into_iter().map(|source| DictionaryEntryCreate::new(source, "")));
            self.backend.update_dictionary(DictionaryEdit::from_dictionary(&dictionary).with_entries(entries))
                .map_err(|_| CommandError::new("dictionary.invalid_update"))?;
        }
        Ok(())
    }

    pub(super) fn workflow_collection_view(&mut self, workflow_id: &str) -> Result<crate::probe::ProbeRunView, CommandError> {
        self.prepare_workflow_collection(workflow_id)?;
        let workflow = self.backend.workflow(workflow_id).map_err(|_| CommandError::new("workflow.invalid"))?;
        if workflow.targets().len() != 1 || workflow.targets()[0].write_dictionary_id().is_none() {
            return Err(CommandError::new("capture.invalid_configuration"));
        }
        let id = self.collection_record_id(workflow_id, workflow.targets()[0].software_id(), workflow.targets()[0].write_dictionary_id().unwrap(), 0)?;
        self.probe_run_summary(&id)
    }

    pub(super) fn prepare_workflow_collection(&mut self, workflow_id: &str) -> Result<(), CommandError> {
        let workflow = self.backend.workflow(workflow_id).map_err(|_| CommandError::new("workflow.invalid"))?;
        let mut collections = BTreeMap::new();
        for (index, target) in workflow.targets().iter().enumerate() {
            let Some(dictionary_id) = target.write_dictionary_id() else { continue };
            self.backend.dictionary(dictionary_id).map_err(|_| CommandError::new("dictionary.not_found"))?;
            let id = self.collection_record_id(workflow_id, target.software_id(), dictionary_id, index)?;
            let excluded = target.dictionary_ids().iter().filter(|id| id.as_ref() != dictionary_id).cloned().collect::<Vec<_>>();
            if let Ok(current) = self.probe_runs.summary(&id) {
                if current.workflow_id() != Some(workflow_id) || current.software_id() != target.software_id() {
                    return Err(CommandError::new("capture.invalid_configuration"));
                }
                let changed = current.dictionary_id() != dictionary_id || current.adapter_ids() != target.adapter_plan().adapter_ids()
                    || current.excluded_dictionary_ids() != excluded;
                if changed && matches!(current.status(), ProbeRunStatus::Running | ProbeRunStatus::Paused) {
                    return Err(CommandError::new("capture.invalid_configuration"));
                }
                if changed || current.name() != workflow.name() || !current.live_preview_enabled() {
                    self.probe_runs.update(&id, ProbeRunUpdate::new(workflow.name(), dictionary_id,
                        target.adapter_plan().adapter_ids().iter().cloned(), true).map_err(probe_run_error)?
                        .with_excluded_dictionaries(excluded).map_err(probe_run_error)?).map_err(probe_run_error)?;
                }
            } else {
                let create = ProbeRunCreate::new(id.clone(), workflow.name(), target.software_id(), dictionary_id,
                    target.adapter_plan().adapter_ids().iter().cloned(), true).map_err(probe_run_error)?
                    .with_excluded_dictionaries(excluded).map_err(probe_run_error)?
                    .with_workflow(workflow_id).map_err(probe_run_error)?;
                self.probe_runs.create(create).map_err(probe_run_error)?;
            }
            collections.insert(target.software_id().into(), self.probe_runs.capture_configuration(&id, DEFAULT_MAX_ENTRIES).map_err(probe_run_error)?);
        }
        if let Some(runtimes) = self.runtimes.as_mut() { runtimes.configure_workflow_collection(workflow_id, collections); }
        Ok(())
    }

    pub(super) fn update_workflow_collection_status(&mut self, workflow_id: &str, running: bool) -> Result<(), CommandError> {
        let workflow = self.backend.workflow(workflow_id).map_err(|_| CommandError::new("workflow.invalid"))?;
        for (index, target) in workflow.targets().iter().enumerate() {
            let Some(dictionary_id) = target.write_dictionary_id() else { continue };
            let running = running && self.workflow_runtime_status.get(workflow_id).is_some_and(|runtime|
                runtime.targets.iter().any(|state| state.software_id.as_ref() == target.software_id() && state.active));
            let id = self.collection_record_id(workflow_id, target.software_id(), dictionary_id, index)?;
            if let Ok(summary) = self.probe_runs.summary(&id) {
                let next = if running && summary.status() == ProbeRunStatus::Paused { ProbeRunStatus::Paused }
                    else if running { ProbeRunStatus::Running } else { ProbeRunStatus::Ready };
                if summary.status() != next { self.probe_runs.set_status(&id, next).map_err(probe_run_error)?; }
            }
        }
        Ok(())
    }
}

#[tauri::command]
pub(super) fn desktop_workflow_collection(
    application: State<'_, Mutex<DesktopApplication>>, workflow_id: String,
) -> Result<crate::probe::ProbeRunView, CommandError> {
    application.lock().map_err(|_| workspace_unavailable())?.workflow_collection_view(&workflow_id)
}

use super::*;

impl DesktopApplication {
    pub(super) fn prepare_exit(&mut self) -> Result<(), CommandError> {
        if self.exit_ready { return Ok(()); }
        // Quiesce reconciliation before stopping so a queued poll cannot inject again.
        self.exiting = true;
        let ids = self.backend.enabled_workflow_ids().iter().cloned()
            .chain(self.workflow_runtime_status.keys().cloned()).collect::<BTreeSet<_>>();
        let mut failed = false;
        for id in ids {
            let Ok(intent) = self.backend.effective_workflow_intent(&id) else { continue };
            let Some(runtimes) = self.runtimes.as_mut() else { continue };
            let runtime = runtimes.stop_workflow(&intent);
            let stopped = !runtime.targets.iter().any(|target| target.active)
                && runtime.errors.values().all(|error| error.code() == "runtime.target_not_found");
            self.workflow_runtime_status.insert(id.clone(), runtime);
            if stopped {
                failed |= self.update_workflow_collection_status(&id, false).is_err();
            } else { failed = true; }
        }
        if let Some(id) = self.active_probe_run_id.clone() {
            failed |= self.disconnect_probe_run(&id).is_err();
        }
        match self.probe_runs.list() {
            Ok(records) => for record in records.iter().filter(|record| record.workflow_id().is_some()) {
                failed |= self.collect_workflow_sources(record.id()).is_err();
            },
            Err(_) => failed = true,
        }
        if failed { Err(CommandError::new("runtime.exit_stop_failed")) } else { self.exit_ready = true; Ok(()) }
    }
}

#[tauri::command]
pub(super) fn desktop_prepare_exit(application: State<'_, Mutex<DesktopApplication>>) -> Result<(), CommandError> {
    application.lock().map_err(|_| workspace_unavailable())?.prepare_exit()
}

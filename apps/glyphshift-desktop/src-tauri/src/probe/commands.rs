use super::*;

#[tauri::command]
pub(crate) fn desktop_probe_runs(
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<Vec<ProbeRunView>, CommandError> {
    application
        .lock()
        .map_err(|_| workspace_unavailable())?
        .probe_run_list()
}

#[tauri::command]
pub(crate) fn desktop_compatible_probe_adapters(
    software_id: String,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<Vec<Box<str>>, CommandError> {
    application
        .lock()
        .map_err(|_| workspace_unavailable())?
        .compatible_probe_adapter_ids(&software_id)
}

#[tauri::command]
pub(crate) fn desktop_delete_probe_runs(
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
pub(crate) fn desktop_update_probe_run(
    request: ProbeRunUpdateRequest,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<ProbeRunView, CommandError> {
    application
        .lock()
        .map_err(|_| workspace_unavailable())?
        .update_probe_run(request)
}

#[tauri::command]
pub(crate) fn desktop_clear_probe_run_entries(
    run_id: String,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<ProbeRunView, CommandError> {
    application
        .lock()
        .map_err(|_| workspace_unavailable())?
        .clear_probe_run_entries(&run_id)
}

#[tauri::command]
pub(crate) fn desktop_resume_probe_run(
    run_id: String,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<ProbeRunView, CommandError> {
    application
        .lock()
        .map_err(|_| runtime_unavailable())?
        .resume_probe_run(&run_id)
}

#[tauri::command]
pub(crate) fn desktop_set_probe_run_paused(
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
pub(crate) fn desktop_refresh_probe_text(
    run_id: String,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<ProbeRunView, CommandError> {
    application
        .lock()
        .map_err(|_| runtime_unavailable())?
        .refresh_probe_text(&run_id)
}

#[tauri::command]
pub(crate) fn desktop_disconnect_probe_run(
    run_id: String,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<ProbeRunView, CommandError> {
    application
        .lock()
        .map_err(|_| runtime_unavailable())?
        .disconnect_probe_run(&run_id)
}

#[tauri::command]
pub(crate) fn desktop_probe_run_summary(
    run_id: String,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<ProbeRunView, CommandError> {
    application
        .lock()
        .map_err(|_| workspace_unavailable())?
        .probe_run_summary(&run_id)
}

#[tauri::command]
pub(crate) fn desktop_probe_run_entries(
    request: ProbeRunQueryRequest,
    application: State<'_, Mutex<DesktopApplication>>,
    settings: State<'_, Mutex<AppSettingsStore>>,
) -> Result<ProbeEntryPage, CommandError> {
    let policy = settings
        .lock()
        .map_err(|_| workspace_unavailable())?
        .current()
        .map_err(|_| workspace_unavailable())?
        .text_filter_policy()
        .clone();
    let cache = std::cell::RefCell::new(
        crate::ai::source_filter_cache()
            .lock()
            .map_err(|_| workspace_unavailable())?,
    );
    cache
        .borrow_mut()
        .configure(&policy)
        .map_err(crate::ai::ai_plan_error)?;
    application
        .lock()
        .map_err(|_| workspace_unavailable())?
        .probe_run_entries_resolved(
            request,
            |source| !cache.borrow_mut().hidden(source),
            |source| cache.borrow_mut().reason(source),
        )
}

#[tauri::command]
pub(crate) fn desktop_edit_probe_translation(
    request: ProbeTranslationEditRequest,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<ProbeRunView, CommandError> {
    application
        .lock()
        .map_err(|_| runtime_unavailable())?
        .edit_probe_translation(request)
}

#[tauri::command]
pub(crate) fn desktop_sync_probe_dictionary_entries(
    request: ProbeDictionarySyncRequest,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<ProbeRunView, CommandError> {
    application
        .lock()
        .map_err(|_| runtime_unavailable())?
        .sync_probe_dictionary_entries(request)
}

#[tauri::command]
pub(crate) fn desktop_bulk_probe_entries(
    request: ProbeBulkRequest,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<ProbeRunView, CommandError> {
    application
        .lock()
        .map_err(|_| runtime_unavailable())?
        .bulk_probe_entries(request)
}

#[tauri::command]
pub(crate) fn desktop_export_probe_run(
    request: ProbeExportRequest,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<(), CommandError> {
    application
        .lock()
        .map_err(|_| workspace_unavailable())?
        .export_probe_run(request)
}

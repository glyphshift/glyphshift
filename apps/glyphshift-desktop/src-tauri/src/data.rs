use super::*;
use tauri_plugin_opener::OpenerExt;

#[tauri::command]
pub(super) fn desktop_open_dictionary_directory(
    app: tauri::AppHandle,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<(), CommandError> {
    let directory = application.lock().map_err(|_| workspace_unavailable())?
        .backend.dictionary_directory();
    let failed = || CommandError::new("data.open_dictionary_failed");
    std::fs::create_dir_all(&directory).map_err(|_| failed())?;
    app.opener().open_path(directory.to_string_lossy().into_owned(), None::<String>)
        .map_err(|_| failed())
}

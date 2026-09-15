use super::*;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

#[derive(Default)]
pub(super) struct WorkflowShortcuts {
    bindings: BTreeMap<Box<str>, Shortcut>,
    pressed: BTreeSet<u32>,
    in_flight: BTreeSet<Box<str>>,
    recording: bool,
    errors: BTreeMap<Box<str>, CommandError>,
}

fn parse(value: &str) -> Result<Option<Shortcut>, CommandError> {
    if value.is_empty() {
        return Ok(None);
    }
    #[cfg(windows)]
    {
        shortcut::parse_global_shortcut(value).map(|(key, _)| Some(key))
    }
    #[cfg(not(windows))]
    {
        Err(CommandError::new("settings.shortcut_unavailable"))
    }
}

pub(super) fn with_binding<T>(
    app: &tauri::AppHandle,
    workflow_id: &str,
    value: &str,
    persist: impl FnOnce() -> Result<T, CommandError>,
) -> Result<T, CommandError> {
    let candidate = parse(value)?;
    let state = app.state::<Mutex<WorkflowShortcuts>>();
    let mut state = state.lock().map_err(|_| runtime_unavailable())?;
    let previous = state.bindings.get(workflow_id).copied();
    if previous == candidate {
        let result = persist()?;
        state.errors.remove(workflow_id);
        return Ok(result);
    }
    if let Some(key) = candidate {
        if app.global_shortcut().is_registered(key) {
            return Err(CommandError::new("workflow.shortcut_conflict"));
        }
        app.global_shortcut()
            .register(key)
            .map_err(|_| CommandError::new("workflow.shortcut_conflict"))?;
    }
    if let Some(key) = previous {
        if app.global_shortcut().unregister(key).is_err() {
            if let Some(candidate) = candidate {
                let _ = app.global_shortcut().unregister(candidate);
            }
            return Err(CommandError::new("settings.shortcut_update_failed"));
        }
    }
    match persist() {
        Ok(result) => {
            state.bindings.remove(workflow_id);
            if let Some(key) = previous {
                state.pressed.remove(&key.id());
            }
            if let Some(key) = candidate {
                state.bindings.insert(workflow_id.into(), key);
            }
            state.errors.remove(workflow_id);
            Ok(result)
        }
        Err(error) => {
            if let Some(key) = candidate {
                let _ = app.global_shortcut().unregister(key);
            }
            if let Some(key) = previous {
                if app.global_shortcut().register(key).is_err() {
                    state.bindings.remove(workflow_id);
                    state.errors.insert(
                        workflow_id.into(),
                        CommandError::new("workflow.shortcut_conflict"),
                    );
                }
            }
            Err(error)
        }
    }
}

pub(super) fn manage(app: &mut tauri::App) {
    app.manage(Mutex::new(WorkflowShortcuts::default()));
    let definitions = app
        .state::<Mutex<DesktopApplication>>()
        .lock()
        .map(|application| {
            application
                .backend
                .snapshot()
                .workflows()
                .iter()
                .map(|workflow| {
                    (
                        Box::<str>::from(workflow.id()),
                        Box::<str>::from(workflow.global_shortcut()),
                    )
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    for (id, value) in definitions {
        if let Err(error) = with_binding(app.handle(), &id, &value, || Ok(())) {
            if let Ok(mut state) = app.state::<Mutex<WorkflowShortcuts>>().lock() {
                state.errors.insert(id, error);
            }
        }
    }
}

pub(super) fn recording(app: &tauri::AppHandle) -> bool {
    app.try_state::<Mutex<WorkflowShortcuts>>()
        .map(|state| {
            state
                .try_lock()
                .map(|state| state.recording)
                .unwrap_or(true)
        })
        .unwrap_or(false)
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkflowShortcutEvent {
    workflow_id: Box<str>,
    result: Option<workflow::WorkflowCommandResult>,
    error: Option<CommandError>,
}

pub(super) fn handle(app: &tauri::AppHandle, key: &Shortcut, event: ShortcutState) {
    let Some(state) = app.try_state::<Mutex<WorkflowShortcuts>>() else {
        return;
    };
    let id = {
        let Ok(mut state) = state.try_lock() else {
            return;
        };
        if event == ShortcutState::Released {
            state.pressed.remove(&key.id());
            return;
        }
        if state.recording || !state.pressed.insert(key.id()) {
            return;
        }
        let Some(id) = state
            .bindings
            .iter()
            .find(|(_, bound)| bound.id() == key.id())
            .map(|(id, _)| id.clone())
        else {
            return;
        };
        if !state.in_flight.insert(id.clone()) {
            return;
        }
        id
    };
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let result = (|| {
            let state = app.state::<Mutex<DesktopApplication>>();
            let mut application = state.lock().map_err(|_| runtime_unavailable())?;
            if application
                .backend
                .enabled_workflow_ids()
                .iter()
                .any(|enabled| enabled == &id)
            {
                application.disable_workflow(&id)
            } else {
                application.enable_workflow(&id, false)
            }
        })();
        let (result, error) = match result {
            Ok(result) => (Some(result), None),
            Err(error) => (None, Some(error)),
        };
        if let Ok(mut state) = app.state::<Mutex<WorkflowShortcuts>>().lock() {
            state.in_flight.remove(&id);
        }
        let _ = app.emit(
            "workflow-shortcut",
            WorkflowShortcutEvent {
                workflow_id: id,
                result,
                error,
            },
        );
    });
}

#[tauri::command]
pub(super) fn desktop_set_shortcut_recording(
    app: tauri::AppHandle,
    recording: bool,
) -> Result<(), CommandError> {
    // Elevation can dispatch WebView IPC while setup is still waiting for UAC.
    let state = app.try_state::<Mutex<WorkflowShortcuts>>().ok_or_else(runtime_unavailable)?;
    let mut state = state.lock().map_err(|_| runtime_unavailable())?;
    state.recording = recording;
    if recording {
        state.pressed.clear();
    }
    Ok(())
}

#[tauri::command]
pub(super) fn desktop_workflow_shortcut_errors(
    app: tauri::AppHandle,
) -> Result<BTreeMap<Box<str>, CommandError>, CommandError> {
    app.try_state::<Mutex<WorkflowShortcuts>>()
        .ok_or_else(runtime_unavailable)?
        .lock()
        .map(|state| state.errors.clone())
        .map_err(|_| runtime_unavailable())
}

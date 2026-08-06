use super::*;

const SOFTWARE_QUICK_CAPTURE_EVENT: &str = "software-quick-capture";
const SOFTWARE_QUICK_CAPTURE_SHORTCUT: &str = "Ctrl+Shift+F8";
const SUPPORTED_TARGET_ARCHITECTURE: &str = "x86_64";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum SoftwarePreflightState {
    Ready,
    AlreadyAdded,
    NotRunning,
    RuntimeUnavailable,
    SelfTarget,
    UnsupportedArchitecture,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SoftwarePreflightView {
    pub(super) executable_path: Box<str>,
    pub(super) executable_name: Box<str>,
    pub(super) suggested_name: Box<str>,
    pub(super) architecture: Box<str>,
    pub(super) running: bool,
    pub(super) can_add: bool,
    pub(super) state: SoftwarePreflightState,
    pub(super) existing_name: Option<Box<str>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(
    tag = "state",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
enum SoftwareQuickCaptureEvent {
    Armed {
        shortcut: &'static str,
    },
    Captured {
        shortcut: &'static str,
        preflight: SoftwarePreflightView,
    },
    Failed {
        shortcut: &'static str,
        error_code: &'static str,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SoftwareQuickCaptureTransition {
    Armed,
    Capture,
}

#[derive(Debug, Default)]
pub(super) struct SoftwareQuickCaptureState {
    pub(super) armed: bool,
    pub(super) shortcut_available: bool,
}

impl SoftwareQuickCaptureState {
    pub(super) fn press(&mut self) -> SoftwareQuickCaptureTransition {
        if self.armed {
            self.armed = false;
            SoftwareQuickCaptureTransition::Capture
        } else {
            self.armed = true;
            SoftwareQuickCaptureTransition::Armed
        }
    }

    pub(super) fn arm(&mut self) -> Result<(), CommandError> {
        if !self.shortcut_available {
            return Err(CommandError::new("software.quick_capture_unavailable"));
        }
        self.armed = true;
        Ok(())
    }

    pub(super) fn cancel(&mut self) {
        self.armed = false;
    }
}
fn display_windows_path(path: &Path) -> String {
    let value = path.to_string_lossy();
    value.strip_prefix(r"\\?\").unwrap_or(&value).to_string()
}

fn same_windows_path(left: &Path, right: &Path) -> bool {
    let left = display_windows_path(left).replace('/', "\\");
    let right = display_windows_path(right).replace('/', "\\");
    left.eq_ignore_ascii_case(&right)
}

pub(super) fn software_preflight_state(
    is_self: bool,
    already_added: bool,
    architecture: &str,
    runtime_ready: bool,
    running: bool,
) -> SoftwarePreflightState {
    if is_self {
        SoftwarePreflightState::SelfTarget
    } else if already_added {
        SoftwarePreflightState::AlreadyAdded
    } else if architecture != SUPPORTED_TARGET_ARCHITECTURE {
        SoftwarePreflightState::UnsupportedArchitecture
    } else if !runtime_ready {
        SoftwarePreflightState::RuntimeUnavailable
    } else if !running {
        SoftwarePreflightState::NotRunning
    } else {
        SoftwarePreflightState::Ready
    }
}

fn software_preflight_error(preflight: &SoftwarePreflightView) -> CommandError {
    match preflight.state {
        SoftwarePreflightState::Ready => CommandError::new("software.preflight_required"),
        SoftwarePreflightState::AlreadyAdded => {
            let error = CommandError::new("software.already_added");
            preflight
                .existing_name
                .as_deref()
                .map_or(error.clone(), |name| error.with_arg("existingName", name))
        }
        SoftwarePreflightState::NotRunning => CommandError::new("software.not_running"),
        SoftwarePreflightState::RuntimeUnavailable => {
            CommandError::new("software.runtime_unavailable")
        }
        SoftwarePreflightState::SelfTarget => CommandError::new("software.self_target"),
        SoftwarePreflightState::UnsupportedArchitecture => {
            CommandError::new("software.unsupported_architecture")
                .with_arg("architecture", preflight.architecture.to_string())
        }
    }
}

fn software_referenced_error(workflow_count: usize, probe_count: usize) -> CommandError {
    CommandError::new("software.referenced")
        .with_arg(
            "workflowCount",
            u64::try_from(workflow_count).unwrap_or(u64::MAX),
        )
        .with_arg("probeCount", u64::try_from(probe_count).unwrap_or(u64::MAX))
}
impl DesktopApplication {
    pub(super) fn software_preflight(
        &self,
        executable_path: impl AsRef<Path>,
    ) -> Result<SoftwarePreflightView, CommandError> {
        let executable = inspect_windows_executable(executable_path)
            .map_err(|_| CommandError::new("software.invalid_executable"))?;
        Ok(self.software_preflight_for(executable))
    }

    pub(super) fn software_preflight_for(
        &self,
        executable: WindowsExecutable,
    ) -> SoftwarePreflightView {
        let executable_path = display_windows_path(executable.path());
        let existing_name = self
            .backend
            .snapshot()
            .software()
            .iter()
            .find(|software| {
                software.executable_path().is_some_and(|candidate| {
                    same_windows_path(Path::new(candidate), executable.path())
                })
            })
            .map(|software| Box::<str>::from(software.name()));
        let is_self = std::env::current_exe()
            .ok()
            .is_some_and(|current| same_windows_path(&current, executable.path()));
        let runtime_ready = self.runtimes.is_some()
            && self.adapters.iter().any(|adapter| {
                adapter
                    .features
                    .iter()
                    .any(|feature| feature.as_ref() == "textObserve")
            });
        let state = software_preflight_state(
            is_self,
            existing_name.is_some(),
            executable.architecture(),
            runtime_ready,
            executable.running(),
        );
        let executable_name = executable.name();
        let suggested_name = Path::new(executable_name)
            .file_stem()
            .and_then(|value| value.to_str())
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(executable_name);
        SoftwarePreflightView {
            executable_path: executable_path.into(),
            executable_name: executable_name.into(),
            suggested_name: suggested_name.into(),
            architecture: executable.architecture().into(),
            running: executable.running(),
            can_add: state == SoftwarePreflightState::Ready,
            state,
            existing_name,
        }
    }

    pub(super) fn add_software(
        &mut self,
        executable_path: String,
    ) -> Result<DesktopProductSnapshot, CommandError> {
        let preflight = self.software_preflight(&executable_path)?;
        if !preflight.can_add {
            return Err(software_preflight_error(&preflight));
        }
        self.backend
            .add_software(ExecutableSelection::new(preflight.executable_path.as_ref()))
            .map_err(|_| CommandError::new("software.invalid_executable"))?;
        Ok(self.snapshot())
    }

    pub(super) fn select_software(
        &mut self,
        extension_id: &str,
    ) -> Result<DesktopProductSnapshot, CommandError> {
        self.backend
            .select_software(extension_id)
            .map_err(|_| CommandError::new("software.select_failed"))?;
        Ok(self.snapshot())
    }

    pub(super) fn remove_software(
        &mut self,
        extension_id: &str,
    ) -> Result<DesktopProductSnapshot, CommandError> {
        let workflow_count = self
            .backend
            .snapshot()
            .workflows()
            .iter()
            .filter(|workflow| {
                workflow
                    .targets()
                    .iter()
                    .any(|target| target.software_id() == extension_id)
            })
            .count();
        let probe_count = self
            .probe_runs
            .list()
            .map_err(|_| CommandError::new("software.delete_failed"))?
            .iter()
            .filter(|probe| probe.software_id() == extension_id)
            .count();
        if workflow_count > 0 || probe_count > 0 {
            return Err(software_referenced_error(workflow_count, probe_count));
        }
        if let Some(runtimes) = self.runtimes.as_mut() {
            runtimes
                .remove_software(extension_id)
                .map_err(|_| CommandError::new("software.runtime_stop_unconfirmed"))?;
        }
        self.backend
            .remove_software(extension_id)
            .map_err(|_| CommandError::new("software.delete_failed"))?;
        Ok(self.snapshot())
    }

    pub(super) fn update_software(
        &mut self,
        extension_id: String,
        display_name: String,
        description: String,
        executable_path: String,
    ) -> Result<DesktopProductSnapshot, CommandError> {
        let edit = SoftwareEdit::new(extension_id.clone(), display_name, executable_path)
            .with_description(description);
        self.backend
            .validate_software_edit(&edit)
            .map_err(|_| CommandError::new("software.invalid_update"))?;
        if let Some(runtimes) = self.runtimes.as_mut() {
            runtimes
                .remove_software(&extension_id)
                .map_err(|_| CommandError::new("software.runtime_stop_unconfirmed"))?;
        }
        self.backend
            .update_software(edit)
            .map_err(|_| CommandError::new("software.invalid_update"))?;
        self.reconcile_enabled_workflows()?;
        Ok(self.snapshot())
    }
}

#[tauri::command]
pub(super) fn desktop_preflight_software(
    executable_path: String,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<SoftwarePreflightView, CommandError> {
    application
        .lock()
        .map_err(|_| workspace_unavailable())?
        .software_preflight(executable_path)
}

#[tauri::command]
pub(super) fn desktop_arm_software_capture(
    capture: State<'_, Mutex<SoftwareQuickCaptureState>>,
) -> Result<&'static str, CommandError> {
    capture.lock().map_err(|_| workspace_unavailable())?.arm()?;
    Ok(SOFTWARE_QUICK_CAPTURE_SHORTCUT)
}

#[tauri::command]
pub(super) fn desktop_cancel_software_capture(
    capture: State<'_, Mutex<SoftwareQuickCaptureState>>,
) -> Result<(), CommandError> {
    capture
        .lock()
        .map_err(|_| workspace_unavailable())?
        .cancel();
    Ok(())
}

#[tauri::command]
pub(super) fn desktop_add_software(
    executable_path: String,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<DesktopProductSnapshot, CommandError> {
    application
        .lock()
        .map_err(|_| workspace_unavailable())?
        .add_software(executable_path)
}

#[tauri::command]
pub(super) fn desktop_select_software(
    extension_id: String,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<DesktopProductSnapshot, CommandError> {
    application
        .lock()
        .map_err(|_| workspace_unavailable())?
        .select_software(&extension_id)
}

#[tauri::command]
pub(super) fn desktop_update_software(
    extension_id: String,
    display_name: String,
    description: String,
    executable_path: String,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<DesktopProductSnapshot, CommandError> {
    application
        .lock()
        .map_err(|_| workspace_unavailable())?
        .update_software(extension_id, display_name, description, executable_path)
}

#[tauri::command]
pub(super) fn desktop_remove_software(
    extension_id: String,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<DesktopProductSnapshot, CommandError> {
    application
        .lock()
        .map_err(|_| workspace_unavailable())?
        .remove_software(&extension_id)
}

#[cfg(windows)]
fn software_quick_capture_shortcut() -> tauri_plugin_global_shortcut::Shortcut {
    use tauri_plugin_global_shortcut::{Code, Modifiers, Shortcut};

    Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::F8)
}

#[cfg(windows)]
fn focus_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[cfg(windows)]
fn rearm_software_quick_capture(app: &tauri::AppHandle) {
    if let Ok(mut capture) = app.state::<Mutex<SoftwareQuickCaptureState>>().lock() {
        capture.armed = true;
    }
}

#[cfg(windows)]
pub(super) fn handle_software_quick_capture_shortcut(app: &tauri::AppHandle) {
    let transition = app
        .state::<Mutex<SoftwareQuickCaptureState>>()
        .lock()
        .ok()
        .map(|mut capture| capture.press());
    match transition {
        Some(SoftwareQuickCaptureTransition::Armed) => {
            let _ = app.emit(
                SOFTWARE_QUICK_CAPTURE_EVENT,
                SoftwareQuickCaptureEvent::Armed {
                    shortcut: SOFTWARE_QUICK_CAPTURE_SHORTCUT,
                },
            );
        }
        Some(SoftwareQuickCaptureTransition::Capture) => {
            let preflight = foreground_windows_executable()
                .map_err(|_| "software.quick_capture_foreground_unavailable")
                .and_then(|executable| {
                    app.state::<Mutex<DesktopApplication>>()
                        .lock()
                        .map_err(|_| "software.quick_capture_failed")
                        .map(|application| application.software_preflight_for(executable))
                });
            match preflight {
                Ok(preflight) if preflight.state == SoftwarePreflightState::SelfTarget => {
                    rearm_software_quick_capture(app);
                    let _ = app.emit(
                        SOFTWARE_QUICK_CAPTURE_EVENT,
                        SoftwareQuickCaptureEvent::Failed {
                            shortcut: SOFTWARE_QUICK_CAPTURE_SHORTCUT,
                            error_code: "software.quick_capture_self",
                        },
                    );
                }
                Ok(preflight) => {
                    let _ = app.emit(
                        SOFTWARE_QUICK_CAPTURE_EVENT,
                        SoftwareQuickCaptureEvent::Captured {
                            shortcut: SOFTWARE_QUICK_CAPTURE_SHORTCUT,
                            preflight,
                        },
                    );
                }
                Err(error_code) => {
                    rearm_software_quick_capture(app);
                    let _ = app.emit(
                        SOFTWARE_QUICK_CAPTURE_EVENT,
                        SoftwareQuickCaptureEvent::Failed {
                            shortcut: SOFTWARE_QUICK_CAPTURE_SHORTCUT,
                            error_code,
                        },
                    );
                }
            }
            focus_main_window(app);
        }
        None => {}
    }
}
pub(super) fn manage_quick_capture(app: &mut tauri::App) {
    app.manage(Mutex::new(SoftwareQuickCaptureState::default()));
    #[cfg(windows)]
    {
        use tauri_plugin_global_shortcut::GlobalShortcutExt;

        let shortcut_available = app
            .global_shortcut()
            .register(software_quick_capture_shortcut())
            .is_ok();
        if let Ok(mut capture) = app.state::<Mutex<SoftwareQuickCaptureState>>().lock() {
            capture.shortcut_available = shortcut_available;
        }
    }
}

use crate::{AppSettings, AppSettingsStore, CommandError};
use std::sync::Mutex;
use tauri::{
    Emitter, Manager,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

const TRAY_ID: &str = "glyphshift-main";

struct TrayMenu {
    show: MenuItem<tauri::Wry>,
    quit: MenuItem<tauri::Wry>,
}

pub(crate) fn restore(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

pub(crate) fn setup(app: &tauri::AppHandle, settings: &AppSettings) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "显示窗口", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &quit])?;
    let icon = app
        .default_window_icon()
        .cloned()
        .ok_or_else(|| tauri::Error::AssetNotFound("window icon".into()))?;
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .tooltip("Glyphshift")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => restore(app),
            "quit" => {
                restore(app);
                // Preserve the editor/AI guards and the normal runtime cleanup path.
                let _ = app.emit("glyphshift-request-exit", ());
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                }
            ) {
                restore(tray.app_handle());
            }
        })
        .build(app)?;
    app.manage(TrayMenu { show, quit });
    set_topmost(app, settings.always_on_top())?;
    update_labels(app, settings);
    Ok(())
}

pub(crate) fn update_labels(app: &tauri::AppHandle, settings: &AppSettings) {
    if let Some(menu) = app.try_state::<TrayMenu>() {
        let _ = menu.show.set_text(if settings.english() {
            "Show window"
        } else {
            "显示窗口"
        });
        let _ = menu
            .quit
            .set_text(if settings.english() { "Quit" } else { "退出" });
    }
}

pub(crate) fn set_topmost(app: &tauri::AppHandle, enabled: bool) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window("main") {
        window.set_always_on_top(enabled)?;
    }
    Ok(())
}

#[tauri::command]
pub(crate) fn desktop_hide_to_tray(app: tauri::AppHandle) -> Result<(), CommandError> {
    if app.tray_by_id(TRAY_ID).is_none() {
        return Err(CommandError::new("settings.window_failed"));
    }
    app.get_webview_window("main")
        .ok_or_else(|| CommandError::new("settings.window_failed"))?
        .hide()
        .map_err(|_| CommandError::new("settings.window_failed"))
}

#[tauri::command]
pub(crate) fn desktop_minimize_window(app: tauri::AppHandle) -> Result<(), CommandError> {
    let to_tray = app
        .state::<Mutex<AppSettingsStore>>()
        .lock()
        .map_err(|_| CommandError::new("settings.unavailable"))?
        .current()
        .map_err(|_| CommandError::new("settings.unavailable"))?
        .minimize_to_tray();
    if to_tray {
        return desktop_hide_to_tray(app);
    }
    app.get_webview_window("main")
        .ok_or_else(|| CommandError::new("settings.window_failed"))?
        .minimize()
        .map_err(|_| CommandError::new("settings.window_failed"))
}

pub(crate) fn window_event(window: &tauri::Window, event: &tauri::WindowEvent) {
    if window.label() != "main"
        || !matches!(event, tauri::WindowEvent::Resized(_))
        || !window.is_minimized().unwrap_or(false)
    {
        return;
    }
    let app = window.app_handle();
    let to_tray = app
        .try_state::<Mutex<AppSettingsStore>>()
        .and_then(|state| {
            state
                .try_lock()
                .ok()
                .and_then(|settings| settings.current().ok())
        })
        .is_some_and(|settings| settings.minimize_to_tray());
    if to_tray && app.tray_by_id(TRAY_ID).is_some() {
        let _ = window.hide();
    }
}

use super::*;

const MAX_SHORTCUT_BYTES: usize = 64;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(super) struct GlobalShortcutProbeView {
    shortcut: Box<str>,
    available: bool,
}

#[cfg(windows)]
pub(super) fn parse_global_shortcut(
    value: &str,
) -> Result<(tauri_plugin_global_shortcut::Shortcut, Box<str>), CommandError> {
    use tauri_plugin_global_shortcut::Modifiers;

    if value.is_empty()
        || value != value.trim()
        || value.len() > MAX_SHORTCUT_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(CommandError::new("settings.shortcut_invalid"));
    }
    let shortcut = value
        .parse::<tauri_plugin_global_shortcut::Shortcut>()
        .map_err(|_| CommandError::new("settings.shortcut_invalid"))?;
    if !shortcut
        .mods
        .intersects(Modifiers::CONTROL | Modifiers::ALT | Modifiers::SUPER)
    {
        return Err(CommandError::new("settings.shortcut_invalid"));
    }
    let mut tokens = Vec::with_capacity(5);
    if shortcut.mods.contains(Modifiers::CONTROL) {
        tokens.push("Ctrl".to_owned());
    }
    if shortcut.mods.contains(Modifiers::ALT) {
        tokens.push("Alt".to_owned());
    }
    if shortcut.mods.contains(Modifiers::SHIFT) {
        tokens.push("Shift".to_owned());
    }
    if shortcut.mods.contains(Modifiers::SUPER) {
        tokens.push("Super".to_owned());
    }
    tokens.push(shortcut.key.to_string());
    Ok((shortcut, tokens.join("+").into()))
}

#[cfg(windows)]
pub(super) fn normalize_global_shortcut(value: &str) -> Result<Box<str>, CommandError> {
    parse_global_shortcut(value).map(|(_, label)| label)
}

#[cfg(not(windows))]
pub(super) fn normalize_global_shortcut(value: &str) -> Result<Box<str>, CommandError> {
    if value.is_empty()
        || value != value.trim()
        || value.len() > MAX_SHORTCUT_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(CommandError::new("settings.shortcut_invalid"));
    }
    Ok(value.into())
}

pub(super) fn probe_global_shortcut(
    app: &tauri::AppHandle,
    value: &str,
    registered_shortcut_id: Option<u32>,
) -> Result<GlobalShortcutProbeView, CommandError> {
    #[cfg(windows)]
    {
        use tauri_plugin_global_shortcut::GlobalShortcutExt;

        let (candidate, label) = parse_global_shortcut(value)?;
        if registered_shortcut_id == Some(candidate.id()) {
            return Ok(GlobalShortcutProbeView {
                shortcut: label,
                available: true,
            });
        }
        if app.global_shortcut().is_registered(candidate) {
            return Ok(GlobalShortcutProbeView {
                shortcut: label,
                available: false,
            });
        }
        let available = match app.global_shortcut().register(candidate) {
            Ok(()) => {
                app.global_shortcut()
                    .unregister(candidate)
                    .map_err(|_| CommandError::new("settings.shortcut_update_failed"))?;
                true
            }
            Err(_) => false,
        };
        Ok(GlobalShortcutProbeView {
            shortcut: label,
            available,
        })
    }
    #[cfg(not(windows))]
    {
        let _ = (app, value, registered_shortcut_id);
        Err(CommandError::new("settings.shortcut_unavailable"))
    }
}

use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;

pub(crate) const APP_SETTINGS_SCHEMA_VERSION: u16 = 1;
pub(crate) const DEFAULT_SOFTWARE_CAPTURE_SHORTCUT: &str = "Ctrl+Shift+F8";
const SETTINGS_FILE_NAME: &str = "app-settings.json";

fn default_software_capture_shortcut() -> Box<str> {
    DEFAULT_SOFTWARE_CAPTURE_SHORTCUT.into()
}

const fn default_confirm_ai_translation() -> bool {
    true
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) enum LocalePreference {
    #[default]
    #[serde(rename = "system")]
    System,
    #[serde(rename = "zh-CN")]
    ZhCn,
    #[serde(rename = "en-US")]
    EnUs,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ThemePreference {
    System,
    #[default]
    Dark,
    Light,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CloseBehavior {
    Minimize,
    #[default]
    Quit,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LegacyAiTranslationBatchSettings {
    #[serde(rename = "maxItemsPerRequest")]
    _max_items_per_request: u16,
    #[serde(default, rename = "maxInputTokensPerRequest", skip_serializing)]
    _legacy_removed_token_budget: Option<u32>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct AppSettings {
    settings_schema_version: u16,
    locale_preference: LocalePreference,
    theme_preference: ThemePreference,
    #[serde(default)]
    launch_at_startup: bool,
    #[serde(default)]
    launch_elevated: bool,
    #[serde(default)]
    close_behavior: CloseBehavior,
    #[serde(default, rename = "interactiveTranslationShortcut", skip_serializing)]
    _legacy_removed_shortcut: Option<Box<str>>,
    #[serde(default = "default_software_capture_shortcut")]
    software_capture_shortcut: Box<str>,
    #[serde(default, rename = "aiTranslationBatch", skip_serializing)]
    _legacy_removed_ai_translation_batch: Option<LegacyAiTranslationBatchSettings>,
    #[serde(default = "default_confirm_ai_translation")]
    confirm_ai_translation: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            settings_schema_version: APP_SETTINGS_SCHEMA_VERSION,
            locale_preference: LocalePreference::default(),
            theme_preference: ThemePreference::default(),
            launch_at_startup: false,
            launch_elevated: false,
            close_behavior: CloseBehavior::default(),
            _legacy_removed_shortcut: None,
            software_capture_shortcut: default_software_capture_shortcut(),
            _legacy_removed_ai_translation_batch: None,
            confirm_ai_translation: true,
        }
    }
}

impl AppSettings {
    pub(crate) const fn launch_at_startup(&self) -> bool {
        self.launch_at_startup
    }

    pub(crate) const fn should_request_elevation(&self, elevated: Option<bool>) -> bool {
        self.launch_elevated && matches!(elevated, Some(false))
    }

    pub(crate) fn software_capture_shortcut(&self) -> &str {
        &self.software_capture_shortcut
    }

    const fn is_valid(&self) -> bool {
        self.settings_schema_version == APP_SETTINGS_SCHEMA_VERSION
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct AppSettingsUpdate {
    locale_preference: LocalePreference,
    theme_preference: ThemePreference,
    launch_at_startup: bool,
    launch_elevated: bool,
    close_behavior: CloseBehavior,
    software_capture_shortcut: Box<str>,
    #[serde(default, rename = "aiTranslationBatch")]
    _legacy_removed_ai_translation_batch: Option<LegacyAiTranslationBatchSettings>,
    confirm_ai_translation: bool,
}

impl AppSettingsUpdate {
    pub(crate) const fn launch_at_startup(&self) -> bool {
        self.launch_at_startup
    }

    pub(crate) fn software_capture_shortcut(&self) -> &str {
        &self.software_capture_shortcut
    }

    pub(crate) fn set_software_capture_shortcut(&mut self, shortcut: Box<str>) {
        self.software_capture_shortcut = shortcut;
    }
}

impl From<AppSettingsUpdate> for AppSettings {
    fn from(update: AppSettingsUpdate) -> Self {
        Self {
            settings_schema_version: APP_SETTINGS_SCHEMA_VERSION,
            locale_preference: update.locale_preference,
            theme_preference: update.theme_preference,
            launch_at_startup: update.launch_at_startup,
            launch_elevated: update.launch_elevated,
            close_behavior: update.close_behavior,
            _legacy_removed_shortcut: None,
            software_capture_shortcut: update.software_capture_shortcut,
            _legacy_removed_ai_translation_batch: None,
            confirm_ai_translation: update.confirm_ai_translation,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SettingsError {
    InvalidData,
    Storage,
    Startup,
}

#[cfg(target_os = "windows")]
pub(crate) fn configure_launch_at_startup(enabled: bool) -> Result<(), SettingsError> {
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};
    use winreg::RegKey;

    const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    const VALUE_NAME: &str = "Glyphshift";

    let run = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags(RUN_KEY, KEY_READ | KEY_WRITE)
        .map_err(|_| SettingsError::Startup)?;
    if enabled {
        let executable = std::env::current_exe().map_err(|_| SettingsError::Startup)?;
        let command = format!("\"{}\"", executable.display());
        run.set_value(VALUE_NAME, &command)
            .map_err(|_| SettingsError::Startup)
    } else {
        match run.delete_value(VALUE_NAME) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(SettingsError::Startup),
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub(crate) const fn configure_launch_at_startup(_enabled: bool) -> Result<(), SettingsError> {
    Err(SettingsError::Startup)
}

pub(crate) struct AppSettingsStore {
    path: PathBuf,
    current: AppSettings,
    load_error: Option<SettingsError>,
}

impl AppSettingsStore {
    pub(crate) fn open(data_root: impl AsRef<Path>) -> Result<Self, SettingsError> {
        let path = data_root.as_ref().join(SETTINGS_FILE_NAME);
        let (current, load_error) = if path.exists() {
            let reader = BufReader::new(File::open(&path).map_err(|_| SettingsError::Storage)?);
            match serde_json::from_reader::<_, AppSettings>(reader) {
                Ok(settings) if settings.is_valid() => (settings, None),
                _ => (AppSettings::default(), Some(SettingsError::InvalidData)),
            }
        } else {
            (AppSettings::default(), None)
        };
        Ok(Self {
            path,
            current,
            load_error,
        })
    }

    pub(crate) fn current(&self) -> Result<AppSettings, SettingsError> {
        self.load_error
            .map_or_else(|| Ok(self.current.clone()), Err)
    }

    pub(crate) const fn launch_at_startup(&self) -> bool {
        self.current.launch_at_startup()
    }

    pub(crate) fn software_capture_shortcut(&self) -> &str {
        self.current.software_capture_shortcut()
    }

    pub(crate) fn update(
        &mut self,
        update: AppSettingsUpdate,
    ) -> Result<AppSettings, SettingsError> {
        let next = AppSettings::from(update);
        if !next.is_valid() {
            return Err(SettingsError::InvalidData);
        }
        self.persist(&next)?;
        self.current = next;
        self.load_error = None;
        Ok(self.current.clone())
    }

    pub(crate) fn update_software_capture_shortcut(
        &mut self,
        shortcut: Box<str>,
    ) -> Result<AppSettings, SettingsError> {
        let mut next = self.current.clone();
        next.software_capture_shortcut = shortcut;
        self.persist(&next)?;
        self.current = next;
        self.load_error = None;
        Ok(self.current.clone())
    }

    fn persist(&self, settings: &AppSettings) -> Result<(), SettingsError> {
        let parent = self.path.parent().ok_or(SettingsError::Storage)?;
        fs::create_dir_all(parent).map_err(|_| SettingsError::Storage)?;
        let mut temporary = NamedTempFile::new_in(parent).map_err(|_| SettingsError::Storage)?;
        {
            let mut writer = BufWriter::new(temporary.as_file_mut());
            serde_json::to_writer_pretty(&mut writer, settings)
                .map_err(|_| SettingsError::Storage)?;
            writer
                .write_all(b"\n")
                .map_err(|_| SettingsError::Storage)?;
            writer.flush().map_err(|_| SettingsError::Storage)?;
        }
        temporary
            .as_file()
            .sync_all()
            .map_err(|_| SettingsError::Storage)?;
        temporary
            .persist(&self.path)
            .map_err(|_| SettingsError::Storage)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn missing_settings_use_system_locale_and_dark_theme_without_creating_a_file() {
        let root = tempdir().expect("temporary settings root");
        let store = AppSettingsStore::open(root.path()).expect("open default settings");

        assert_eq!(store.current(), Ok(AppSettings::default()));
        assert_eq!(
            store.current().expect("default settings").theme_preference,
            ThemePreference::Dark
        );
        assert!(!root.path().join(SETTINGS_FILE_NAME).exists());
    }

    #[test]
    fn settings_round_trip_through_the_single_store_interface() {
        let root = tempdir().expect("temporary settings root");
        let mut store = AppSettingsStore::open(root.path()).expect("open settings store");

        let saved = store
            .update(AppSettingsUpdate {
                locale_preference: LocalePreference::EnUs,
                theme_preference: ThemePreference::Light,
                launch_at_startup: true,
                launch_elevated: true,
                close_behavior: CloseBehavior::Minimize,
                software_capture_shortcut: "Ctrl+Alt+KeyS".into(),
                _legacy_removed_ai_translation_batch: None,
                confirm_ai_translation: false,
            })
            .expect("save settings");
        let reopened = AppSettingsStore::open(root.path()).expect("reopen settings store");

        assert!(saved.should_request_elevation(Some(false)));
        assert!(!saved.should_request_elevation(Some(true)));
        assert!(!saved.should_request_elevation(None));
        assert_eq!(saved.software_capture_shortcut(), "Ctrl+Alt+KeyS");
        assert!(!saved.confirm_ai_translation);
        assert_eq!(reopened.current(), Ok(saved));
    }

    #[test]
    fn unknown_schema_and_enum_values_are_rejected() {
        let root = tempdir().expect("temporary settings root");
        fs::write(
            root.path().join(SETTINGS_FILE_NAME),
            r#"{"settingsSchemaVersion":2,"localePreference":"fr-FR","themePreference":"dark"}"#,
        )
        .expect("write invalid settings");

        let mut store = AppSettingsStore::open(root.path()).expect("open with safe defaults");
        assert_eq!(store.current(), Err(SettingsError::InvalidData));

        let recovered = store
            .update(AppSettingsUpdate {
                locale_preference: LocalePreference::ZhCn,
                theme_preference: ThemePreference::Dark,
                launch_at_startup: false,
                launch_elevated: false,
                close_behavior: CloseBehavior::Quit,
                software_capture_shortcut: DEFAULT_SOFTWARE_CAPTURE_SHORTCUT.into(),
                _legacy_removed_ai_translation_batch: None,
                confirm_ai_translation: true,
            })
            .expect("replace invalid settings");
        assert_eq!(store.current(), Ok(recovered));
    }

    #[test]
    fn legacy_settings_gain_safe_application_behavior_defaults() {
        let root = tempdir().expect("temporary settings root");
        fs::write(
            root.path().join(SETTINGS_FILE_NAME),
            r#"{"settingsSchemaVersion":1,"localePreference":"zh-CN","themePreference":"dark","interactiveTranslationShortcut":"Ctrl+Shift+F9","aiTranslationBatch":{"maxItemsPerRequest":75,"maxInputTokensPerRequest":16000}}"#,
        )
        .expect("write legacy settings");

        let settings = AppSettingsStore::open(root.path())
            .expect("open legacy settings")
            .current()
            .expect("legacy settings remain valid");

        assert!(!settings.launch_at_startup);
        assert!(!settings.launch_elevated);
        assert!(!settings.should_request_elevation(Some(false)));
        assert_eq!(settings.close_behavior, CloseBehavior::Quit);
        assert_eq!(
            settings.software_capture_shortcut(),
            DEFAULT_SOFTWARE_CAPTURE_SHORTCUT
        );
        assert!(settings.confirm_ai_translation);
        let serialized = serde_json::to_value(settings).expect("serialize migrated settings");
        assert!(serialized.get("interactiveTranslationShortcut").is_none());
        assert!(serialized.get("aiTranslationBatch").is_none());
    }

    #[test]
    fn removed_global_ai_batch_policy_is_ignored_on_update() {
        let root = tempdir().expect("temporary settings root");
        let mut store = AppSettingsStore::open(root.path()).expect("open settings store");

        let saved = store
            .update(AppSettingsUpdate {
                locale_preference: LocalePreference::ZhCn,
                theme_preference: ThemePreference::Dark,
                launch_at_startup: false,
                launch_elevated: false,
                close_behavior: CloseBehavior::Quit,
                software_capture_shortcut: DEFAULT_SOFTWARE_CAPTURE_SHORTCUT.into(),
                _legacy_removed_ai_translation_batch: Some(LegacyAiTranslationBatchSettings {
                    _max_items_per_request: 75,
                    _legacy_removed_token_budget: None,
                }),
                confirm_ai_translation: true,
            })
            .expect("save settings without the removed batch policy");

        assert_eq!(store.current(), Ok(saved.clone()));
        assert!(serde_json::to_value(saved)
            .expect("serialize settings")
            .get("aiTranslationBatch")
            .is_none());
    }
}

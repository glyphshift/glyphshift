use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;

pub(crate) const APP_SETTINGS_SCHEMA_VERSION: u16 = 1;
const SETTINGS_FILE_NAME: &str = "app-settings.json";

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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct AppSettings {
    settings_schema_version: u16,
    locale_preference: LocalePreference,
    theme_preference: ThemePreference,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            settings_schema_version: APP_SETTINGS_SCHEMA_VERSION,
            locale_preference: LocalePreference::default(),
            theme_preference: ThemePreference::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct AppSettingsUpdate {
    locale_preference: LocalePreference,
    theme_preference: ThemePreference,
}

impl From<AppSettingsUpdate> for AppSettings {
    fn from(update: AppSettingsUpdate) -> Self {
        Self {
            settings_schema_version: APP_SETTINGS_SCHEMA_VERSION,
            locale_preference: update.locale_preference,
            theme_preference: update.theme_preference,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SettingsError {
    InvalidData,
    Storage,
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
                Ok(settings) if settings.settings_schema_version == APP_SETTINGS_SCHEMA_VERSION => {
                    (settings, None)
                }
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

    pub(crate) fn update(
        &mut self,
        update: AppSettingsUpdate,
    ) -> Result<AppSettings, SettingsError> {
        let next = AppSettings::from(update);
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
            })
            .expect("save settings");
        let reopened = AppSettingsStore::open(root.path()).expect("reopen settings store");

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
            })
            .expect("replace invalid settings");
        assert_eq!(store.current(), Ok(recovered));
    }
}

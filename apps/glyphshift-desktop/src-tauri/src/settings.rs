use glyphshift_ai_translation::FilterPolicy;
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;

pub(crate) const APP_SETTINGS_SCHEMA_VERSION: u16 = 1;
pub(crate) const DEFAULT_SOFTWARE_CAPTURE_SHORTCUT: &str = "Ctrl+Shift+F8";
const SETTINGS_FILE_NAME: &str = "app-settings.json";

#[test]
fn favorite_defaults_use_installed_aliases_once_and_preserve_explicit_choices() {
    let root = tempfile::tempdir().unwrap();
    let installed = ["微软雅黑", "Microsoft YaHei", "arial", "Synthetic Serif"].map(Box::<str>::from);
    let mut store = AppSettingsStore::open(root.path()).unwrap();
    store.initialize_favorite_fonts(&installed).unwrap();
    assert_eq!(store.current.favorite_fonts, ["Microsoft YaHei", "arial"]);
    store.current.favorite_fonts.clear();
    store.persist(&store.current).unwrap();
    let mut reopened = AppSettingsStore::open(root.path()).unwrap();
    reopened.initialize_favorite_fonts(&installed).unwrap();
    assert!(reopened.current.favorite_fonts.is_empty());
    reopened.current.favorite_fonts = vec!["Synthetic Serif".into()];
    reopened.persist(&reopened.current).unwrap();
    let mut existing = AppSettingsStore::open(root.path()).unwrap();
    existing.initialize_favorite_fonts(&installed).unwrap();
    assert_eq!(existing.current.favorite_fonts, ["Synthetic Serif"]);
}

fn default_translation_languages() -> Vec<String> {
    ["zh-cn", "zh-tw", "en", "ja", "ko", "fr", "de", "es", "pt", "ru", "it", "ar", "th", "vi", "id", "tr", "pl", "uk", "hi", "nl"]
        .into_iter().map(str::to_owned).collect()
}

fn valid_language(language: &str) -> bool {
    let language = language.trim();
    let mut parts = language.split('-');
    let first = parts.next().unwrap_or_default();
    let primary_valid = ((2..=8).contains(&first.len()) && first.bytes().all(|c| c.is_ascii_alphabetic())) || first.eq_ignore_ascii_case("x");
    primary_valid && language.len() <= 63 && !language.eq_ignore_ascii_case("auto") && !language.eq_ignore_ascii_case("x")
        && parts.all(|part| (1..=8).contains(&part.len()) && part.bytes().all(|c| c.is_ascii_alphanumeric()))
}

fn valid_catalog(values: &[String], limit: usize, max_length: usize) -> bool {
    let mut seen = std::collections::HashSet::new();
    values.len() <= limit && values.iter().all(|value| !value.trim().is_empty()
        && value.chars().count() <= max_length && !value.chars().any(char::is_control)
        && seen.insert(value.trim().to_lowercase()))
}

fn normalize_catalog(value: Option<&serde_json::Value>, limit: usize, max_length: usize) -> Vec<String> {
    let mut result = Vec::new();
    let mut seen = std::collections::HashSet::new();
    if let Some(values) = value.and_then(serde_json::Value::as_array) {
        for value in values.iter().take(limit).filter_map(serde_json::Value::as_str) {
            let value = value.trim();
            if !value.is_empty() && value.chars().count() <= max_length && !value.chars().any(char::is_control) && seen.insert(value.to_lowercase()) {
                result.push(value.to_owned());
            }
        }
    }
    result
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LanguageFallbackFont {
    language: String,
    font_family: String,
}

impl LanguageFallbackFont {
    fn normalized(mut self) -> Self {
        self.language = self.language.trim().to_ascii_lowercase();
        self.font_family = self.font_family.trim().to_owned();
        self
    }

    fn is_valid(&self) -> bool {
        valid_language(&self.language)
            && !self.font_family.trim().is_empty() && self.font_family.trim().chars().count() <= 128
            && !self.font_family.chars().any(char::is_control)
    }
}

fn valid_font_fallbacks(rows: &[LanguageFallbackFont]) -> bool {
    let mut languages = std::collections::HashSet::new();
    rows.len() <= 64 && rows.iter().all(|row| row.is_valid() && languages.insert(row.language.trim().to_ascii_lowercase()))
}

fn normalize_font_fallbacks(value: Option<&serde_json::Value>) -> Vec<LanguageFallbackFont> {
    let mut result: Vec<LanguageFallbackFont> = Vec::new();
    if let Some(rows) = value.and_then(serde_json::Value::as_array) {
        for value in rows.iter().take(64) {
            if let Ok(row) = serde_json::from_value::<LanguageFallbackFont>(value.clone()) {
                let row = row.normalized();
                if row.is_valid() && !result.iter().any(|existing| existing.language == row.language) {
                    result.push(row);
                }
            }
        }
    }
    result
}

#[cfg(test)]
mod font_fallback_tests {
    use super::*;

    #[test]
    fn catalogs_preserve_explicit_empty_lists_and_isolate_invalid_entries() {
        let settings = normalize_persisted_settings(&serde_json::json!({
            "translationLanguages": [], "recentSoftwareIds": [],
            "favoriteFonts": ["Synthetic Serif", "synthetic serif", "", 12, "Synthetic Mono"]
        }));
        assert!(settings.translation_languages.is_empty());
        assert_eq!(settings.recent_software_ids, Some(Vec::new()));
        assert_eq!(settings.favorite_fonts, ["Synthetic Serif", "Synthetic Mono"]);
        assert_eq!(normalize_persisted_settings(&serde_json::json!({})).translation_languages.len(), 20);
        assert_eq!(normalize_persisted_settings(&serde_json::json!({})).recent_software_ids, None);
    }

    #[test]
    fn managed_catalogs_survive_other_preference_updates_and_reopen() {
        let root = tempfile::tempdir().unwrap();
        let mut store = AppSettingsStore::open(root.path()).unwrap();
        let mut value = serde_json::to_value(AppSettings::default()).unwrap();
        value.as_object_mut().unwrap().remove("settingsSchemaVersion");
        value["translationLanguages"] = serde_json::json!(["JA", "eo"]);
        value["favoriteFonts"] = serde_json::json!(["Synthetic Mono"]);
        value["recentSoftwareIds"] = serde_json::json!([]);
        store.update(serde_json::from_value(value.clone()).unwrap()).unwrap();
        value["themePreference"] = serde_json::json!("light");
        store.update(serde_json::from_value(value).unwrap()).unwrap();
        let restored = AppSettingsStore::open(root.path()).unwrap();
        assert_eq!(restored.current.translation_languages, ["ja", "eo"]);
        assert_eq!(restored.current.favorite_fonts, ["Synthetic Mono"]);
        assert_eq!(restored.current.recent_software_ids, Some(Vec::new()));
    }

    #[test]
    fn persisted_font_rows_are_isolated_and_language_duplicates_are_case_insensitive() {
        let settings = normalize_persisted_settings(&serde_json::json!({
            "themePreference": "light",
            "languageFallbackFonts": [
                {"language":" JA ","fontFamily":" Synthetic Sans "},
                {"language":"ja","fontFamily":"Synthetic Serif"},
                {"language":"not a code","fontFamily":"Synthetic Sans"},
                {"language":"eo","fontFamily":"Synthetic Serif"},
                {"language":"ko","fontFamily":""}
            ]
        }));
        assert_eq!(settings.theme_preference, ThemePreference::Light);
        assert_eq!(settings.language_fallback_fonts.len(), 2);
        assert_eq!(settings.language_fallback_fonts[0].language, "ja");
        assert_eq!(settings.language_fallback_fonts[0].font_family, "Synthetic Sans");
    }

    #[test]
    fn font_preferences_survive_reopen_and_invalid_updates_do_not_replace_them() {
        let root = tempfile::tempdir().unwrap();
        let mut store = AppSettingsStore::open(root.path()).unwrap();
        let mut value = serde_json::to_value(AppSettings::default()).unwrap();
        value.as_object_mut().unwrap().remove("settingsSchemaVersion");
        value["languageFallbackFonts"] = serde_json::json!([{"language":"zh-CN","fontFamily":"Synthetic Sans"}]);
        store.update(serde_json::from_value(value.clone()).unwrap()).unwrap();
        let reopened = AppSettingsStore::open(root.path()).unwrap();
        assert_eq!(reopened.current.language_fallback_fonts[0].language, "zh-cn");
        value["languageFallbackFonts"] = serde_json::json!([
            {"language":"ja","fontFamily":"Synthetic Sans"},
            {"language":"JA","fontFamily":"Synthetic Serif"}
        ]);
        assert!(store.update(serde_json::from_value(value).unwrap()).is_err());
        assert_eq!(store.current.language_fallback_fonts, reopened.current.language_fallback_fonts);
    }
}

fn default_check_updates() -> bool { true }

fn default_auto_complete_interval() -> u16 {
    10
}

fn default_software_capture_shortcut() -> Box<str> {
    DEFAULT_SOFTWARE_CAPTURE_SHORTCUT.into()
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

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
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
    #[serde(default = "default_software_capture_shortcut")]
    software_capture_shortcut: Box<str>,
    #[serde(default = "default_auto_complete_interval")]
    auto_complete_interval_seconds: u16,
    #[serde(default = "default_check_updates")]
    check_updates_on_startup: bool,
    #[serde(default)]
    text_filter_policy: FilterPolicy,
    #[serde(default)]
    language_fallback_fonts: Vec<LanguageFallbackFont>,
    #[serde(default)]
    favorite_fonts: Vec<String>,
    #[serde(default = "default_translation_languages")]
    translation_languages: Vec<String>,
    #[serde(default)]
    recent_software_ids: Option<Vec<String>>,
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
            software_capture_shortcut: default_software_capture_shortcut(),
            auto_complete_interval_seconds: 10,
            check_updates_on_startup: true,
            text_filter_policy: FilterPolicy::default(),
            language_fallback_fonts: Vec::new(),
            favorite_fonts: Vec::new(),
            translation_languages: default_translation_languages(),
            recent_software_ids: None,
        }
    }
}

impl AppSettings {
    pub(crate) fn text_filter_policy(&self) -> &FilterPolicy { &self.text_filter_policy }
    pub(crate) const fn launch_at_startup(&self) -> bool {
        self.launch_at_startup
    }

    pub(crate) const fn should_request_elevation(&self, elevated: Option<bool>) -> bool {
        self.launch_elevated && matches!(elevated, Some(false))
    }

    pub(crate) fn software_capture_shortcut(&self) -> &str {
        &self.software_capture_shortcut
    }

    fn is_valid(&self) -> bool {
        self.settings_schema_version == APP_SETTINGS_SCHEMA_VERSION
            && self.auto_complete_interval_seconds <= 60
            && self.text_filter_policy.hidden_sources(&[]).is_ok()
            && valid_font_fallbacks(&self.language_fallback_fonts)
            && valid_catalog(&self.favorite_fonts, 64, 128)
            && valid_catalog(&self.translation_languages, 128, 63)
            && self.translation_languages.iter().all(|code| valid_language(code))
            && self.recent_software_ids.as_ref().is_none_or(|ids| valid_catalog(ids, 20, 128))

    }
}

fn normalize_persisted_settings(value: &serde_json::Value) -> AppSettings {
    let Some(object) = value.as_object() else {
        return AppSettings::default();
    };
    let locale_preference = match object
        .get("localePreference")
        .and_then(|value| value.as_str())
    {
        Some("zh-CN") => LocalePreference::ZhCn,
        Some("en-US") => LocalePreference::EnUs,
        _ => LocalePreference::System,
    };
    let theme_preference = match object
        .get("themePreference")
        .and_then(|value| value.as_str())
    {
        Some("system") => ThemePreference::System,
        Some("light") => ThemePreference::Light,
        _ => ThemePreference::Dark,
    };
    let close_behavior = match object.get("closeBehavior").and_then(|value| value.as_str()) {
        Some("minimize") => CloseBehavior::Minimize,
        _ => CloseBehavior::Quit,
    };
    let software_capture_shortcut = object
        .get("softwareCaptureShortcut")
        .and_then(|value| value.as_str())
        .filter(|value| !value.trim().is_empty())
        .map_or_else(default_software_capture_shortcut, Into::into);
    AppSettings {
        settings_schema_version: APP_SETTINGS_SCHEMA_VERSION,
        locale_preference,
        theme_preference,
        launch_at_startup: object
            .get("launchAtStartup")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        launch_elevated: object
            .get("launchElevated")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        close_behavior,
        software_capture_shortcut,
        language_fallback_fonts: normalize_font_fallbacks(object.get("languageFallbackFonts")),
        favorite_fonts: normalize_catalog(object.get("favoriteFonts"), 64, 128),
        translation_languages: object.get("translationLanguages").filter(|value| value.is_array()).map_or_else(default_translation_languages, |value| normalize_catalog(Some(value), 128, 63).into_iter().map(|code| code.to_ascii_lowercase()).filter(|code| valid_language(code)).collect()),
        recent_software_ids: object.get("recentSoftwareIds").filter(|value| value.is_array()).map(|value| normalize_catalog(Some(value), 20, 128)),
        text_filter_policy: object.get("textFilterPolicy").and_then(|value| serde_json::from_value(value.clone()).ok()).filter(|policy: &FilterPolicy| policy.hidden_sources(&[]).is_ok()).unwrap_or_default(),
        check_updates_on_startup: object.get("checkUpdatesOnStartup").and_then(serde_json::Value::as_bool).unwrap_or(true),
        auto_complete_interval_seconds: object
            .get("autoCompleteIntervalSeconds")
            .and_then(serde_json::Value::as_u64)
            .filter(|value| (0..=60).contains(value))
            .map_or(10, |value| value as u16),
    }
}

fn preserve_invalid_settings(path: &Path) {
    let backup = path.with_extension("invalid.json");
    if !backup.exists() {
        let _ = fs::copy(path, backup);
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
    #[serde(default = "default_auto_complete_interval")]
    auto_complete_interval_seconds: u16,
    #[serde(default = "default_check_updates")]
    check_updates_on_startup: bool,
    #[serde(default)]
    text_filter_policy: FilterPolicy,
    #[serde(default)]
    language_fallback_fonts: Vec<LanguageFallbackFont>,
    #[serde(default)]
    favorite_fonts: Vec<String>,
    #[serde(default = "default_translation_languages")]
    translation_languages: Vec<String>,
    #[serde(default)]
    recent_software_ids: Option<Vec<String>>,
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
            software_capture_shortcut: update.software_capture_shortcut,
            auto_complete_interval_seconds: update.auto_complete_interval_seconds,
            check_updates_on_startup: update.check_updates_on_startup,
            text_filter_policy: update.text_filter_policy,
            language_fallback_fonts: update.language_fallback_fonts.into_iter().map(LanguageFallbackFont::normalized).collect(),
            favorite_fonts: update.favorite_fonts.into_iter().map(|font| font.trim().to_owned()).collect(),
            translation_languages: update.translation_languages.into_iter().map(|code| code.trim().to_ascii_lowercase()).collect(),
            recent_software_ids: update.recent_software_ids,
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
    use winreg::RegKey;
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};

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
    initialize_favorite_fonts: bool,
}

impl AppSettingsStore {
    pub(crate) fn open(data_root: impl AsRef<Path>) -> Result<Self, SettingsError> {
        let path = data_root.as_ref().join(SETTINGS_FILE_NAME);
        let (current, load_error) = if path.exists() {
            let reader = BufReader::new(File::open(&path).map_err(|_| SettingsError::Storage)?);
            match serde_json::from_reader::<_, serde_json::Value>(reader) {
                Ok(value) => (normalize_persisted_settings(&value), None),
                Err(_) => {
                    preserve_invalid_settings(&path);
                    (AppSettings::default(), None)
                }
            }
        } else {
            (AppSettings::default(), None)
        };
        let initialize_favorite_fonts = !fs::read(&path).ok()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
            .is_some_and(|value| value.get("favoriteFonts").is_some_and(serde_json::Value::is_array));
        let mut store = Self { path, current, load_error, initialize_favorite_fonts };
        // Promote only the default profile's old policy once; profile selection no longer changes it.
        let has_global_policy = fs::read(&store.path).ok()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
            .is_some_and(|value| value.get("textFilterPolicy").is_some());
        if !has_global_policy {
            let legacy = fs::read(data_root.as_ref().join("ai-profiles.json")).ok()
                .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok());
            let policy = legacy.as_ref().and_then(|value| {
                let default_id = value.get("defaultProfileId")?.as_str()?;
                let profile = value.get("profiles")?.as_array()?.iter()
                    .find(|profile| profile.get("id").and_then(|id| id.as_str()) == Some(default_id))?;
                serde_json::from_value::<FilterPolicy>(profile.get("filterPolicy")?.clone()).ok()
            }).filter(|policy| policy.hidden_sources(&[]).is_ok());
            if let Some(policy) = policy {
                store.current.text_filter_policy = policy;
                store.persist(&store.current)?;
            }
        }
        Ok(store)
    }

    pub(crate) fn current(&self) -> Result<AppSettings, SettingsError> {
        self.load_error
            .map_or_else(|| Ok(self.current.clone()), Err)
    }

    pub(crate) fn initialize_favorite_fonts(&mut self, installed: &[Box<str>]) -> Result<(), SettingsError> {
        if !self.initialize_favorite_fonts || installed.is_empty() { return Ok(()); }
        let groups: Vec<Vec<String>> = serde_json::from_str(include_str!("../../src/defaultFavoriteFonts.json"))
            .map_err(|_| SettingsError::InvalidData)?;
        let mut next = self.current.clone();
        for aliases in groups {
            if next.favorite_fonts.iter().any(|font| aliases.iter().any(|alias| font.eq_ignore_ascii_case(alias))) { continue; }
            if let Some(font) = aliases.iter().find_map(|alias| installed.iter().find(|font| font.eq_ignore_ascii_case(alias))) {
                next.favorite_fonts.push(font.to_string());
            }
        }
        self.persist(&next)?;
        self.current = next;
        self.initialize_favorite_fonts = false;
        Ok(())
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
    fn update_check_defaults_on_and_opt_out_survives_reload() {
        assert!(normalize_persisted_settings(&serde_json::json!({})).check_updates_on_startup);
        let root = tempdir().unwrap();
        let mut store = AppSettingsStore::open(root.path()).unwrap();
        let mut value = serde_json::to_value(AppSettings::default()).unwrap();
        value.as_object_mut().unwrap().remove("settingsSchemaVersion");
        value["checkUpdatesOnStartup"] = false.into();
        store.update(serde_json::from_value(value).unwrap()).unwrap();
        let restored = AppSettingsStore::open(root.path()).unwrap();
        assert!(!restored.current().unwrap().check_updates_on_startup);
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
                auto_complete_interval_seconds: 10,
                check_updates_on_startup: true,
            text_filter_policy: FilterPolicy::default(),
            language_fallback_fonts: Vec::new(),
            favorite_fonts: Vec::new(),
            translation_languages: default_translation_languages(),
            recent_software_ids: None,
                software_capture_shortcut: "Ctrl+Alt+KeyS".into(),
            })
            .expect("save settings");
        let reopened = AppSettingsStore::open(root.path()).expect("reopen settings store");

        assert!(saved.should_request_elevation(Some(false)));
        assert!(!saved.should_request_elevation(Some(true)));
        assert!(!saved.should_request_elevation(None));
        assert_eq!(saved.software_capture_shortcut(), "Ctrl+Alt+KeyS");
        assert!(
            serde_json::to_value(&saved)
                .expect("serialize settings")
                .get("confirmAiTranslation")
                .is_none()
        );
        assert_eq!(reopened.current(), Ok(saved));
    }

    #[test]
    fn unknown_schema_and_enum_values_fall_back_without_blocking_settings() {
        let root = tempdir().expect("temporary settings root");
        fs::write(
            root.path().join(SETTINGS_FILE_NAME),
            r#"{"settingsSchemaVersion":2,"localePreference":"fr-FR","themePreference":"dark"}"#,
        )
        .expect("write invalid settings");

        let mut store = AppSettingsStore::open(root.path()).expect("open with safe defaults");
        assert_eq!(store.current(), Ok(AppSettings::default()));

        let recovered = store
            .update(AppSettingsUpdate {
                locale_preference: LocalePreference::ZhCn,
                theme_preference: ThemePreference::Dark,
                launch_at_startup: false,
                launch_elevated: false,
                close_behavior: CloseBehavior::Quit,
                auto_complete_interval_seconds: 10,
                check_updates_on_startup: true,
            text_filter_policy: FilterPolicy::default(),
            language_fallback_fonts: Vec::new(),
            favorite_fonts: Vec::new(),
            translation_languages: default_translation_languages(),
            recent_software_ids: None,
                software_capture_shortcut: DEFAULT_SOFTWARE_CAPTURE_SHORTCUT.into(),
            })
            .expect("replace invalid settings");
        assert_eq!(store.current(), Ok(recovered));
    }

    #[test]
    fn settings_keep_valid_fields_when_other_fields_are_unknown_or_invalid() {
        let root = tempdir().expect("temporary settings root");
        fs::write(
            root.path().join(SETTINGS_FILE_NAME),
            r#"{
  "settingsSchemaVersion": 999,
  "localePreference": "en-US",
  "themePreference": 42,
  "launchAtStartup": true,
  "launchElevated": "invalid",
  "closeBehavior": "future-option",
  "softwareCaptureShortcut": "Ctrl+Alt+KeyS",
  "unknownFutureField": { "enabled": true }
}"#,
        )
        .expect("write partially invalid settings");

        let settings = AppSettingsStore::open(root.path())
            .expect("open settings store")
            .current()
            .expect("salvage valid settings fields");

        assert_eq!(settings.locale_preference, LocalePreference::EnUs);
        assert_eq!(settings.theme_preference, ThemePreference::Dark);
        assert!(settings.launch_at_startup);
        assert!(!settings.launch_elevated);
        assert_eq!(settings.close_behavior, CloseBehavior::Quit);
        assert_eq!(settings.software_capture_shortcut(), "Ctrl+Alt+KeyS");
    }

    #[test]
    fn malformed_settings_fall_back_without_deleting_the_original_file() {
        let root = tempdir().expect("temporary settings root");
        fs::write(root.path().join(SETTINGS_FILE_NAME), b"{").expect("write malformed settings");

        let settings = AppSettingsStore::open(root.path())
            .expect("open malformed settings safely")
            .current()
            .expect("use safe settings defaults");

        assert_eq!(settings, AppSettings::default());
        assert!(root.path().join("app-settings.json").exists());
        assert!(root.path().join("app-settings.invalid.json").exists());
    }

    #[test]
    fn missing_fields_use_safe_application_behavior_defaults_and_unknown_fields_are_ignored() {
        let root = tempdir().expect("temporary settings root");
        fs::write(
            root.path().join(SETTINGS_FILE_NAME),
            r#"{"settingsSchemaVersion":1,"localePreference":"zh-CN","themePreference":"dark","unknownShortcut":"Ctrl+Shift+F9","unknownObject":{"count":75},"unknownBoolean":false}"#,
        )
        .expect("write partial settings");

        let settings = AppSettingsStore::open(root.path())
            .expect("open partial settings")
            .current()
            .expect("partial settings remain valid");

        assert!(!settings.launch_at_startup);
        assert!(!settings.launch_elevated);
        assert!(!settings.should_request_elevation(Some(false)));
        assert_eq!(settings.close_behavior, CloseBehavior::Quit);
        assert_eq!(
            settings.software_capture_shortcut(),
            DEFAULT_SOFTWARE_CAPTURE_SHORTCUT
        );
        let serialized = serde_json::to_value(settings).expect("serialize normalized settings");
        assert!(serialized.get("unknownShortcut").is_none());
        assert!(serialized.get("unknownObject").is_none());
        assert!(serialized.get("unknownBoolean").is_none());
    }
}

#[cfg(test)]
mod auto_complete_tests {
    use super::*;
    #[test]
    fn interval_defaults_and_bounds_are_preserved() {
        assert_eq!(
            normalize_persisted_settings(&serde_json::json!({})).auto_complete_interval_seconds,
            10
        );
        for value in [-1, 61, 3601] {
            assert_eq!(
                normalize_persisted_settings(
                    &serde_json::json!({"autoCompleteIntervalSeconds": value})
                )
                .auto_complete_interval_seconds,
                10
            );
        }
        for value in [0, 1, 4, 60] {
            let settings = normalize_persisted_settings(&serde_json::json!({"autoCompleteIntervalSeconds": value}));
            assert_eq!(settings.auto_complete_interval_seconds, value);
            assert!(settings.is_valid());
        }
        let root = tempfile::tempdir().expect("settings root");
        let path = root.path().join("settings.json");
        let settings =
            normalize_persisted_settings(&serde_json::json!({"autoCompleteIntervalSeconds": 25}));
        fs::write(&path, serde_json::to_vec(&settings).expect("serialize")).expect("write");
        let value = serde_json::from_slice(&fs::read(path).expect("read")).expect("JSON");
        assert_eq!(
            normalize_persisted_settings(&value).auto_complete_interval_seconds,
            25
        );
    }
}

#[cfg(test)]
mod global_filter_tests {
    use super::*;
    #[test]
    fn default_profile_filter_migrates_once_and_other_profiles_cannot_change_it() {
        let root = tempfile::tempdir().unwrap();
        let legacy = serde_json::json!({"defaultProfileId":"chosen", "profiles":[
            {"id":"chosen", "filterPolicy":{"skipTextContainingDigits":true}},
            {"id":"other", "filterPolicy":{"skipTextContainingDigits":false}}
        ]});
        fs::write(root.path().join("ai-profiles.json"), legacy.to_string()).unwrap();
        let store = AppSettingsStore::open(root.path()).unwrap();
        assert_eq!(store.current().unwrap().text_filter_policy().hidden_sources(&["Chapter 2".into()]).unwrap(), vec![true]);
        let mut changed = legacy;
        changed["defaultProfileId"] = serde_json::json!("other");
        fs::write(root.path().join("ai-profiles.json"), changed.to_string()).unwrap();
        let reopened = AppSettingsStore::open(root.path()).unwrap();
        assert_eq!(reopened.current().unwrap().text_filter_policy().hidden_sources(&["Chapter 2".into()]).unwrap(), vec![true]);
    }
}

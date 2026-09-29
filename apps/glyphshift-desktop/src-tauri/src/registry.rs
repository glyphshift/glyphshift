//! Desktop Registry configuration is operator-owned, never supplied by web commands.
use super::*;
use crate::dictionary::{
    DictionaryCatalogInstallRequest, DictionaryCatalogPageView, DictionaryCatalogQueryRequest,
    DictionaryCatalogReleaseView, DictionaryReplacementRequest,
};
use glyphshift_registry_client::{
    DictionarySearch, Error, RegistryClient, ReleaseRequest, TrustStore, read_bounded,
};

pub(super) struct DesktopRegistry {
    config_path: Option<PathBuf>,
    data_root: PathBuf,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Configuration {
    schema: String,
    origin: String,
    catalog_id: String,
    trust_file: PathBuf,
    ca_file: Option<PathBuf>,
}

impl DesktopRegistry {
    pub(super) fn from_environment(data_root: PathBuf) -> Self {
        Self {
            config_path: std::env::var_os("GLYPHSHIFT_REGISTRY_CONFIG").map(PathBuf::from),
            data_root,
        }
    }
    fn load(&self) -> Result<Option<Configuration>, CommandError> {
        self.config_path
            .as_deref()
            .map(Configuration::read)
            .transpose()
            .map_err(registry_error)
    }
}

impl Configuration {
    fn read(path: &Path) -> Result<Self, Error> {
        if !path.is_absolute() {
            return Err(Error::Configuration);
        }
        let mut config: Self =
            serde_json::from_slice(&read_bounded(path, 8192)?).map_err(|_| Error::Configuration)?;
        if config.schema != "glyphshift.desktop-registry/1" {
            return Err(Error::Configuration);
        }
        DictionaryReleaseKey::new(config.catalog_id.as_str(), "synthetic.validation", "1.0.0")
            .map_err(|_| Error::Configuration)?;
        let parent = path.parent().ok_or(Error::Configuration)?;
        if config.trust_file.as_os_str().is_empty() {
            return Err(Error::Configuration);
        }
        if !config.trust_file.is_absolute() {
            config.trust_file = parent.join(&config.trust_file);
        }
        if let Some(ca) = &mut config.ca_file {
            if ca.as_os_str().is_empty() {
                return Err(Error::Configuration);
            }
            if !ca.is_absolute() {
                *ca = parent.join(&*ca);
            }
        }
        Ok(config)
    }
    fn trust(&self) -> Result<TrustStore, Error> {
        TrustStore::from_json(&read_bounded(&self.trust_file, 65536)?)
    }
    fn client(&self) -> Result<RegistryClient, Error> {
        let ca = self
            .ca_file
            .as_deref()
            .map(|p| read_bounded(p, 65536))
            .transpose()?;
        RegistryClient::with_ca(&self.origin, self.trust()?, ca.as_deref())
    }
    fn release(&self, request: &DictionaryCatalogInstallRequest) -> Result<ReleaseRequest, Error> {
        if request.catalog_id.as_ref() != self.catalog_id
            || matches!(
                request.replacement,
                DictionaryReplacementRequest::ReplaceAny
            )
        {
            return Err(Error::Configuration);
        }
        ReleaseRequest::new(
            "dictionary",
            &request.dictionary_id,
            &request.release_version,
            &request.publisher_identity,
        )
    }
}

pub(super) fn registry_error(error: Error) -> CommandError {
    CommandError::new(match error {
        Error::Configuration => "dictionary.catalog_invalid",
        Error::InvalidProof => "dictionary.signature_invalid",
        Error::UntrustedKey => "dictionary.publisher_untrusted",
        Error::IdentityMismatch => "dictionary.publisher_identity_mismatch",
        Error::InvalidArtifact => "dictionary.payload_invalid",
        Error::Unavailable => "dictionary.catalog_unavailable",
        Error::NotFound => "dictionary.release_missing",
        Error::TooLarge => "dictionary.artifact_too_large",
        Error::LocalChangesConflict => "dictionary.local_changes_conflict",
        Error::Storage => "dictionary.installation_storage_failure",
    })
}

pub(super) fn query(
    app: &tauri::AppHandle,
    request: DictionaryCatalogQueryRequest,
) -> Result<DictionaryCatalogPageView, CommandError> {
    let config = app.state::<DesktopRegistry>().load()?;
    let Some(config) = config else {
        return app
            .state::<Mutex<DesktopApplication>>()
            .lock()
            .map_err(|_| workspace_unavailable())?
            .query_dictionary_catalog(request);
    };
    let query = DictionarySearch {
        text: request.text.into(),
        source_locale: request.source_locale.map(Into::into),
        target_locale: request.target_locale.map(Into::into),
        tag: request.tag.map(Into::into),
        cursor: request.cursor.map(Into::into),
        size: request.page_size.unwrap_or(50),
    };
    let page = config
        .client()
        .map_err(registry_error)?
        .search_dictionaries(&query)
        .map_err(registry_error)?;
    Ok(DictionaryCatalogPageView {
        releases: page
            .items
            .into_iter()
            .map(|entry| DictionaryCatalogReleaseView {
                catalog_id: config.catalog_id.clone().into(),
                dictionary_id: entry.package_id.into(),
                release_version: entry.version.into(),
                effective_presentation_locale: entry.source_locale.clone().into(),
                source_locale: entry.source_locale.into(),
                target_locale: entry.target_locale.into(),
                name: entry.name.into(),
                summary: entry.summary.into(),
                tags: entry.tags.into_iter().map(Into::into).collect(),
                publisher_identity: entry.publisher_user_key.into(),
            })
            .collect(),
        next_cursor: page.next_cursor.map(Into::into),
    })
}

pub(super) fn install(
    app: &tauri::AppHandle,
    request: DictionaryCatalogInstallRequest,
) -> Result<DesktopProductSnapshot, CommandError> {
    let configuration = app.state::<DesktopRegistry>();
    let config = configuration
        .load()?
        .ok_or_else(|| CommandError::new("dictionary.catalog_unavailable"))?;
    let release = config.release(&request).map_err(registry_error)?;
    {
        let state = app.state::<Mutex<DesktopApplication>>();
        let application = state.lock().map_err(|_| workspace_unavailable())?;
        application.ensure_ai_dictionary_writable(&request.dictionary_id)?;
        if application.exiting {
            return Err(workspace_unavailable());
        }
    }
    // All TLS, proof requests and downloading take place outside the application mutex.
    let prepared = config
        .client()
        .map_err(registry_error)?
        .prepare_dictionary(&release)
        .map_err(registry_error)?;
    let state = app.state::<Mutex<DesktopApplication>>();
    let mut application = state.lock().map_err(|_| workspace_unavailable())?;
    application.ensure_ai_dictionary_writable(&request.dictionary_id)?;
    if application.exiting || configuration.load()?.as_ref() != Some(&config) {
        return Err(workspace_unavailable());
    }
    // Reload trust and re-check on-disk edits at commit, including edits made during download.
    prepared
        .install(
            config.trust().map_err(registry_error)?,
            &config.catalog_id,
            &configuration.data_root,
            request.replacement.into(),
        )
        .map_err(registry_error)?;
    application
        .backend
        .reload_dictionaries()
        .map_err(|_| registry_error(Error::Storage))?;
    application.reconcile_enabled_workflows()?;
    Ok(application.snapshot())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configuration_and_install_identity_fail_closed() {
        let base = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../local-test/evidence/desktop-registry");
        std::fs::create_dir_all(&base).unwrap();
        let root = tempfile::tempdir_in(base).unwrap();
        let root = root.path().canonicalize().unwrap();
        let path = root.join("registry.json");
        let json = serde_json::json!({"schema":"glyphshift.desktop-registry/1","origin":"https://registry.invalid","catalogId":"glyphshift.official","trustFile":"trust.json"});
        std::fs::write(&path, json.to_string()).unwrap();
        let config = Configuration::read(&path).unwrap();
        assert_eq!(config.trust_file, root.join("trust.json"));
        assert!(config.client().is_err()); // Missing trust cannot silently switch to unverified downloads.
        let mut request = DictionaryCatalogInstallRequest {
            catalog_id: "glyphshift.official".into(),
            dictionary_id: "fixture.dictionary".into(),
            release_version: "1.0.0".into(),
            publisher_identity: "Abcdef23".into(),
            replacement: DictionaryReplacementRequest::RejectExisting,
        };
        assert_eq!(config.release(&request).unwrap().publisher(), "Abcdef23");
        request.catalog_id = "foreign".into();
        assert!(config.release(&request).is_err());
        request.catalog_id = "glyphshift.official".into();
        request.publisher_identity = "".into();
        assert!(config.release(&request).is_err());
        request.publisher_identity = "Abcdef23".into();
        request.replacement = DictionaryReplacementRequest::ReplaceAny;
        assert!(config.release(&request).is_err());
        let mut unexpected = json.clone();
        unexpected["downloadUrl"] = serde_json::json!("https://foreign.invalid");
        std::fs::write(&path, unexpected.to_string()).unwrap();
        assert!(Configuration::read(&path).is_err());
        std::fs::write(&path,r#"{"schema":"glyphshift.desktop-registry/1","origin":"https://a.invalid","origin":"https://b.invalid","catalogId":"glyphshift.official","trustFile":"trust.json"}"#).unwrap();
        assert!(Configuration::read(&path).is_err());
        assert!(Configuration::read(Path::new("relative.json")).is_err());
        assert!(
            DesktopRegistry {
                config_path: None,
                data_root: root
            }
            .load()
            .unwrap()
            .is_none()
        );
    }
}

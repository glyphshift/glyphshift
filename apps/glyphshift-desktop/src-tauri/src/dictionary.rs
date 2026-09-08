use super::*;
use crate::probe::probe_run_error;

pub(super) fn dictionary_distribution_error(error: DictionaryDistributionError) -> CommandError {
    let code = match error {
        DictionaryDistributionError::InvalidCatalog => "dictionary.catalog_invalid",
        DictionaryDistributionError::CatalogUnavailable => "dictionary.catalog_unavailable",
        DictionaryDistributionError::ReleaseMissing => "dictionary.release_missing",
        DictionaryDistributionError::ArtifactTooLarge => "dictionary.artifact_too_large",
        DictionaryDistributionError::SizeMismatch => "dictionary.artifact_size_mismatch",
        DictionaryDistributionError::DigestMismatch => "dictionary.artifact_digest_mismatch",
        DictionaryDistributionError::InvalidSignature => "dictionary.signature_invalid",
        DictionaryDistributionError::UntrustedPublisher => "dictionary.publisher_untrusted",
        DictionaryDistributionError::PublisherIdentityMismatch => {
            "dictionary.publisher_identity_mismatch"
        }
        DictionaryDistributionError::TrustUnavailable => "dictionary.trust_unavailable",
        DictionaryDistributionError::InvalidPayload => "dictionary.payload_invalid",
        DictionaryDistributionError::ReleaseIdentityMismatch => {
            "dictionary.release_identity_mismatch"
        }
        DictionaryDistributionError::LocalChangesConflict => "dictionary.local_changes_conflict",
        DictionaryDistributionError::StorageFailure => "dictionary.installation_storage_failure",
    };
    CommandError::new(code)
}

pub(super) fn dictionary_import_error(error: BackendError) -> CommandError {
    match error {
        BackendError::DuplicateDictionary(id) => CommandError::new("dictionary.import_duplicate")
            .with_arg("dictionaryId", id.to_string()),
        BackendError::InvalidInput(_) | BackendError::InvalidArtifact(_) => {
            CommandError::new("dictionary.import_invalid")
        }
        BackendError::Storage(_) => CommandError::new("dictionary.import_failed"),
        _ => CommandError::new("dictionary.import_failed"),
    }
}

pub(super) fn dictionary_export_error(error: BackendError) -> CommandError {
    match error {
        BackendError::UnknownDictionary(id) => {
            CommandError::new("dictionary.not_found").with_arg("dictionaryId", id.to_string())
        }
        _ => CommandError::new("dictionary.export_failed"),
    }
}

fn dictionary_referenced_error(
    workflow_names: BTreeSet<Box<str>>,
    probe_names: BTreeSet<Box<str>>,
) -> CommandError {
    CommandError::new("dictionary.referenced")
        .with_arg(
            "workflowNames",
            workflow_names.into_iter().collect::<Vec<_>>(),
        )
        .with_arg("probeNames", probe_names.into_iter().collect::<Vec<_>>())
}

struct OfflineDictionaryCatalog;

impl DictionaryDistributionPort for OfflineDictionaryCatalog {
    fn query(&mut self, _query: &CatalogQuery) -> Result<CatalogSourcePage, CatalogPortError> {
        Err(CatalogPortError::Unavailable)
    }

    fn release(&mut self, _key: &DictionaryReleaseKey) -> Result<CatalogRelease, CatalogPortError> {
        Err(CatalogPortError::Unavailable)
    }

    fn fetch(
        &mut self,
        _download_url: &str,
        _byte_limit: u64,
    ) -> Result<Vec<u8>, CatalogPortError> {
        Err(CatalogPortError::Unavailable)
    }
}

struct OfflineArtifactTrustVerifier;

impl ArtifactTrustVerifier for OfflineArtifactTrustVerifier {
    fn verify(
        &mut self,
        _statement: &ArtifactStatement,
        _signature: &SignatureEnvelope,
    ) -> Result<PublisherIdentity, TrustVerifierError> {
        Err(TrustVerifierError::Unavailable)
    }
}

pub(super) fn offline_dictionary_distribution(
    data_root: &std::path::Path,
) -> Result<DictionaryDistribution, String> {
    let store = FileDictionaryInstallStore::open(data_root)
        .map_err(|error| format!("dictionary installation startup: {error:?}"))?;
    Ok(DictionaryDistribution::new(
        Box::new(OfflineDictionaryCatalog),
        Box::new(OfflineArtifactTrustVerifier),
        Box::new(store),
        Box::new(SystemInstallationClock),
    ))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DictionaryCatalogQueryRequest {
    pub(super) text: Box<str>,
    pub(super) source_locale: Option<Box<str>>,
    pub(super) target_locale: Option<Box<str>>,
    pub(super) tag: Option<Box<str>>,
    pub(super) cursor: Option<Box<str>>,
    pub(super) page_size: Option<u16>,
    pub(super) requested_presentation_locale: Box<str>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(super) struct DictionaryCatalogReleaseView {
    pub(super) catalog_id: Box<str>,
    pub(super) dictionary_id: Box<str>,
    pub(super) release_version: Box<str>,
    pub(super) source_locale: Box<str>,
    pub(super) target_locale: Box<str>,
    pub(super) effective_presentation_locale: Box<str>,
    pub(super) name: Box<str>,
    pub(super) summary: Box<str>,
    pub(super) tags: Vec<Box<str>>,
    pub(super) publisher_identity: Box<str>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(super) struct DictionaryCatalogPageView {
    pub(super) releases: Vec<DictionaryCatalogReleaseView>,
    pub(super) next_cursor: Option<Box<str>>,
}

impl From<CatalogPage> for DictionaryCatalogPageView {
    fn from(page: CatalogPage) -> Self {
        Self {
            releases: page
                .releases()
                .iter()
                .map(|release| DictionaryCatalogReleaseView {
                    catalog_id: release.key().catalog_id().into(),
                    dictionary_id: release.key().dictionary_id().into(),
                    release_version: release.key().release_version().into(),
                    source_locale: release.source_locale().into(),
                    target_locale: release.target_locale().into(),
                    effective_presentation_locale: release.effective_presentation_locale().into(),
                    name: release.presentation().name().into(),
                    summary: release.presentation().summary().into(),
                    tags: release.presentation().tags().to_vec(),
                    publisher_identity: release.publisher_identity().as_str().into(),
                })
                .collect(),
            next_cursor: page.next_cursor().map(Into::into),
        }
    }
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum DictionaryReplacementRequest {
    RejectExisting,
    ReplaceVerified,
    ReplaceAny,
}

impl From<DictionaryReplacementRequest> for DictionaryReplacementPolicy {
    fn from(value: DictionaryReplacementRequest) -> Self {
        match value {
            DictionaryReplacementRequest::RejectExisting => Self::RejectExisting,
            DictionaryReplacementRequest::ReplaceVerified => Self::ReplaceVerified,
            DictionaryReplacementRequest::ReplaceAny => Self::ReplaceAny,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DictionaryCatalogInstallRequest {
    pub(super) catalog_id: Box<str>,
    pub(super) dictionary_id: Box<str>,
    pub(super) release_version: Box<str>,
    pub(super) replacement: DictionaryReplacementRequest,
}

impl DesktopApplication {
    pub(super) fn dictionary_detail(
        &self,
        dictionary_id: &str,
    ) -> Result<DictionaryView, CommandError> {
        self.backend
            .dictionary(dictionary_id)
            .cloned()
            .map_err(|_| {
                CommandError::new("dictionary.not_found").with_arg("dictionaryId", dictionary_id)
            })
    }

    pub(super) fn query_dictionary_catalog(
        &mut self,
        request: DictionaryCatalogQueryRequest,
    ) -> Result<DictionaryCatalogPageView, CommandError> {
        let mut query = CatalogQuery::new(request.text);
        if let Some(source_locale) = request.source_locale {
            query = query.with_source_locale(source_locale);
        }
        if let Some(target_locale) = request.target_locale {
            query = query.with_target_locale(target_locale);
        }
        if let Some(tag) = request.tag {
            query = query.with_tag(tag);
        }
        if let Some(cursor) = request.cursor {
            query = query.with_cursor(cursor);
        }
        if let Some(page_size) = request.page_size {
            query = query.with_page_size(page_size);
        }
        self.dictionary_distribution
            .query(&query, &request.requested_presentation_locale)
            .map(DictionaryCatalogPageView::from)
            .map_err(dictionary_distribution_error)
    }

    pub(super) fn install_dictionary_release(
        &mut self,
        request: DictionaryCatalogInstallRequest,
    ) -> Result<DesktopProductSnapshot, CommandError> {
        self.ensure_ai_dictionary_writable(&request.dictionary_id)?;
        let release = DictionaryReleaseKey::new(
            request.catalog_id,
            request.dictionary_id,
            request.release_version,
        )
        .map_err(|_| CommandError::new("dictionary.catalog_invalid"))?;
        self.dictionary_distribution
            .install(&InstallRequest::new(release, request.replacement.into()))
            .map_err(dictionary_distribution_error)?;
        self.backend.reload_dictionaries().map_err(|_| {
            dictionary_distribution_error(DictionaryDistributionError::StorageFailure)
        })?;
        self.reconcile_enabled_workflows()?;
        Ok(self.snapshot())
    }

    pub(super) fn create_dictionary(
        &mut self,
        create: DictionaryCreate,
    ) -> Result<DesktopProductSnapshot, CommandError> {
        self.backend
            .create_dictionary(create)
            .map_err(|_| CommandError::new("dictionary.invalid_create"))?;
        Ok(self.snapshot())
    }

    pub(super) fn import_dictionary_file(
        &mut self,
        input_path: PathBuf,
    ) -> Result<DesktopProductSnapshot, CommandError> {
        if self.ai_locked_dictionary_id.is_some() {
            return Err(CommandError::new("dictionary.ai_translation_locked"));
        }
        self.backend
            .import_dictionary_file(input_path)
            .map_err(dictionary_import_error)?;
        Ok(self.snapshot())
    }

    pub(super) fn export_dictionary_file(
        &self,
        dictionary_id: &str,
        output_path: PathBuf,
    ) -> Result<(), CommandError> {
        self.backend
            .export_dictionary_file(dictionary_id, output_path)
            .map_err(dictionary_export_error)
    }

    pub(super) fn update_dictionary(
        &mut self,
        edit: DictionaryEdit,
    ) -> Result<DesktopProductSnapshot, CommandError> {
        self.ensure_ai_dictionary_writable(edit.id())?;
        self.backend
            .update_dictionary(edit)
            .map_err(|_| CommandError::new("dictionary.invalid_update"))?;
        self.reconcile_enabled_workflows()?;
        Ok(self.snapshot())
    }

    pub(super) fn delete_dictionaries(
        &mut self,
        dictionary_ids: &[Box<str>],
    ) -> Result<DesktopProductSnapshot, CommandError> {
        for dictionary_id in dictionary_ids {
            self.ensure_ai_dictionary_writable(dictionary_id)?;
        }
        let workflow_names = self
            .backend
            .snapshot()
            .workflows()
            .iter()
            .filter(|workflow| {
                workflow.dictionary_ids().iter().any(|candidate| {
                    dictionary_ids
                        .iter()
                        .any(|id| id.as_ref() == candidate.as_ref())
                })
            })
            .map(|workflow| Box::<str>::from(workflow.name()))
            .collect::<BTreeSet<_>>();
        let probe_names = self
            .probe_runs
            .list()
            .map_err(probe_run_error)?
            .into_iter()
            .filter(|run| {
                dictionary_ids
                    .iter()
                    .any(|id| id.as_ref() == run.dictionary_id() || run.excluded_dictionary_ids().contains(id))
            })
            .map(|run| Box::<str>::from(run.name()))
            .collect::<BTreeSet<_>>();
        if !workflow_names.is_empty() || !probe_names.is_empty() {
            return Err(dictionary_referenced_error(workflow_names, probe_names));
        }
        self.backend
            .delete_dictionaries(dictionary_ids.iter().map(AsRef::as_ref))
            .map_err(|_| CommandError::new("dictionary.referenced"))?;
        Ok(self.snapshot())
    }
}

#[tauri::command]
pub(super) fn desktop_dictionary(
    dictionary_id: String,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<DictionaryView, CommandError> {
    application
        .lock()
        .map_err(|_| workspace_unavailable())?
        .dictionary_detail(&dictionary_id)
}

#[tauri::command]
pub(super) fn desktop_query_dictionary_catalog(
    request: DictionaryCatalogQueryRequest,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<DictionaryCatalogPageView, CommandError> {
    application
        .lock()
        .map_err(|_| workspace_unavailable())?
        .query_dictionary_catalog(request)
}

#[tauri::command]
pub(super) fn desktop_install_dictionary_release(
    request: DictionaryCatalogInstallRequest,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<DesktopProductSnapshot, CommandError> {
    application
        .lock()
        .map_err(|_| workspace_unavailable())?
        .install_dictionary_release(request)
}

#[tauri::command]
pub(super) fn desktop_create_dictionary(
    create: DictionaryCreate,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<DesktopProductSnapshot, CommandError> {
    application
        .lock()
        .map_err(|_| workspace_unavailable())?
        .create_dictionary(create)
}

#[tauri::command]
pub(super) fn desktop_import_dictionary(
    input_path: PathBuf,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<DesktopProductSnapshot, CommandError> {
    application
        .lock()
        .map_err(|_| workspace_unavailable())?
        .import_dictionary_file(input_path)
}

#[tauri::command]
pub(super) fn desktop_export_dictionary(
    dictionary_id: String,
    output_path: PathBuf,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<(), CommandError> {
    application
        .lock()
        .map_err(|_| workspace_unavailable())?
        .export_dictionary_file(&dictionary_id, output_path)
}

#[tauri::command]
pub(super) fn desktop_update_dictionary(
    edit: DictionaryEdit,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<DesktopProductSnapshot, CommandError> {
    application
        .lock()
        .map_err(|_| workspace_unavailable())?
        .update_dictionary(edit)
}

#[tauri::command]
pub(super) fn desktop_delete_dictionaries(
    dictionary_ids: Vec<String>,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<DesktopProductSnapshot, CommandError> {
    let dictionary_ids = dictionary_ids
        .into_iter()
        .map(Box::<str>::from)
        .collect::<Vec<_>>();
    application
        .lock()
        .map_err(|_| workspace_unavailable())?
        .delete_dictionaries(&dictionary_ids)
}

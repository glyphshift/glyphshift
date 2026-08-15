use super::*;
use glyphshift_ai_translation::{
    AiProfileCatalog, AiProfileDraft, AiProfileError, AiProfileView, AiTranslation,
    CancellationOutcome, CredentialVault, CredentialVaultError, PlanError, ReqwestHttpTransport,
    TranslationItem, TranslationJobError, TranslationJobId, TranslationJobSnapshot,
    TranslationPlan, TranslationPlanRequest,
};
use std::path::Path;
use std::sync::Arc;

const AI_CREDENTIAL_SERVICE: &str = "Glyphshift AI Profiles";

pub(super) struct DesktopAiState {
    profiles: AiProfileCatalog,
    translation: AiTranslation,
}

impl DesktopAiState {
    pub(super) fn open(data_root: &Path) -> Result<Self, String> {
        let profiles = AiProfileCatalog::open(data_root, system_credential_vault())
            .map_err(|error| format!("AI profile startup: {error:?}"))?;
        let mut translation = AiTranslation::new();
        translation.register_first_release_http_providers(Arc::new(ReqwestHttpTransport));
        Ok(Self {
            profiles,
            translation,
        })
    }

    fn profiles_view(&self) -> Result<AiProfilesView, CommandError> {
        Ok(AiProfilesView {
            default_profile_id: self.profiles.default_profile_id().map(Into::into),
            profiles: self.profiles.profiles().map_err(ai_profile_error)?,
        })
    }

    fn save_profile(
        &mut self,
        request: AiProfileSaveRequest,
    ) -> Result<AiProfilesView, CommandError> {
        let profile = self
            .profiles
            .save_profile(request.profile)
            .map_err(ai_profile_error)?;
        if request.make_default {
            self.profiles
                .set_default_profile(profile.id())
                .map_err(ai_profile_error)?;
        }
        self.profiles_view()
    }

    fn set_default_profile(&mut self, profile_id: &str) -> Result<AiProfilesView, CommandError> {
        self.profiles
            .set_default_profile(profile_id)
            .map_err(ai_profile_error)?;
        self.profiles_view()
    }

    fn delete_profile(&mut self, profile_id: &str) -> Result<AiProfilesView, CommandError> {
        self.profiles
            .delete_profile(profile_id)
            .map_err(ai_profile_error)?;
        self.profiles_view()
    }

    fn selected_profile_id(&self, requested: Option<&str>) -> Result<Box<str>, CommandError> {
        requested
            .or_else(|| self.profiles.default_profile_id())
            .map(Into::into)
            .ok_or_else(|| CommandError::new("ai.profile_required"))
    }

    fn plan_translation(
        &mut self,
        request: AiTranslationPlanRequest,
    ) -> Result<TranslationPlan, CommandError> {
        let profile_id = request.profile_id.clone();
        let items = request.items.into_iter().map(|item| {
            let AiTranslationItemRequest {
                item_id,
                source,
                translation,
                ignored,
            } = item;
            let item = match translation {
                Some(translation) => TranslationItem::translated(item_id, source, translation),
                None => TranslationItem::untranslated(item_id, source),
            };
            if ignored {
                item.ignored()
            } else {
                item
            }
        });
        self.plan_request(
            profile_id.as_deref(),
            TranslationPlanRequest::new(
                request.scope_id,
                request.snapshot_revision,
                request.source_locale,
                request.target_locale,
                items,
            ),
        )
    }

    fn plan_request(
        &mut self,
        profile_id: Option<&str>,
        request: TranslationPlanRequest,
    ) -> Result<TranslationPlan, CommandError> {
        let profile_id = self.selected_profile_id(profile_id)?;
        let profile = self
            .profiles
            .profile(&profile_id)
            .map_err(ai_profile_error)?;
        self.translation
            .plan_translation(request.with_filter_policy(profile.filter_policy().clone()))
            .map_err(ai_plan_error)
    }

    fn start_translation(
        &mut self,
        request: AiTranslationStartRequest,
    ) -> Result<TranslationJobId, CommandError> {
        let profile_id = self.selected_profile_id(request.profile_id.as_deref())?;
        let profile = self
            .profiles
            .resolve_profile(&profile_id)
            .map_err(ai_profile_error)?;
        self.translation
            .start_translation(&request.plan_token, profile)
            .map_err(ai_job_error)
    }

    fn translation_job(&self, job_id: &str) -> Result<TranslationJobSnapshot, CommandError> {
        self.translation
            .translation_job(&TranslationJobId::new(job_id))
            .map_err(ai_job_error)
    }

    fn cancel_translation(&self, job_id: &str) -> Result<CancellationOutcome, CommandError> {
        self.translation
            .cancel_translation(&TranslationJobId::new(job_id))
            .map_err(ai_job_error)
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(super) struct AiProfilesView {
    default_profile_id: Option<Box<str>>,
    profiles: Vec<AiProfileView>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct AiProfileSaveRequest {
    profile: AiProfileDraft,
    #[serde(default)]
    make_default: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct AiTranslationItemRequest {
    item_id: Box<str>,
    source: Box<str>,
    translation: Option<Box<str>>,
    #[serde(default)]
    ignored: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct AiTranslationPlanRequest {
    profile_id: Option<Box<str>>,
    scope_id: Box<str>,
    snapshot_revision: u64,
    source_locale: Box<str>,
    target_locale: Box<str>,
    items: Vec<AiTranslationItemRequest>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct AiTranslationStartRequest {
    plan_token: Box<str>,
    profile_id: Option<Box<str>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ProbeAiPlanRequest {
    run_id: Box<str>,
    profile_id: Option<Box<str>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ProbeAiTranslationResult {
    pub(super) item_id: Box<str>,
    pub(super) source: Box<str>,
    pub(super) translation: Box<str>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ProbeAiApplyRequest {
    pub(super) run_id: Box<str>,
    pub(super) snapshot_revision: u64,
    pub(super) results: Vec<ProbeAiTranslationResult>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ProbeAiApplyView {
    pub(super) probe: probe::ProbeRunView,
    pub(super) applied_count: usize,
    pub(super) skipped_count: usize,
}

impl DesktopApplication {
    pub(super) fn probe_ai_plan_request(
        &mut self,
        run_id: &str,
    ) -> Result<TranslationPlanRequest, CommandError> {
        let summary = self
            .probe_runs
            .summary(run_id)
            .map_err(probe::probe_run_error)?;
        let dictionary = self
            .backend
            .dictionary(summary.dictionary_id())
            .cloned()
            .map_err(|_| CommandError::new("dictionary.not_found"))?;
        let snapshot = self.probe_dictionary_snapshot(dictionary.id())?;
        let mut page_number = 1;
        let mut row_number = 0_usize;
        let mut items = Vec::new();
        loop {
            let query = ProbeQuery::new(
                "",
                page_number,
                glyphshift_capture::MAX_PROBE_QUERY_PAGE_SIZE,
            )
            .map_err(probe::probe_run_error)?;
            let page = self
                .probe_runs
                .query_entries(run_id, &query, &snapshot)
                .map_err(probe::probe_run_error)?;
            for row in page.rows() {
                row_number = row_number.saturating_add(1);
                let item_id = format!("probe-row-{row_number}");
                let item = if row.translation().trim().is_empty() {
                    TranslationItem::untranslated(item_id, row.source())
                } else {
                    TranslationItem::translated(item_id, row.source(), row.translation())
                };
                items.push(
                    if row.state() == glyphshift_capture::ProbeEntryState::Ignored {
                        item.ignored()
                    } else {
                        item
                    },
                );
            }
            if items.len() >= page.total() {
                break;
            }
            page_number = page_number.saturating_add(1);
        }
        Ok(TranslationPlanRequest::new(
            format!("probe:{run_id}"),
            dictionary.revision(),
            dictionary.metadata().source_locale(),
            dictionary.metadata().target_locale(),
            items,
        ))
    }

    pub(super) fn apply_probe_ai_results(
        &mut self,
        request: ProbeAiApplyRequest,
    ) -> Result<ProbeAiApplyView, CommandError> {
        let summary = self
            .probe_runs
            .summary(&request.run_id)
            .map_err(probe::probe_run_error)?;
        let dictionary = self
            .backend
            .dictionary(summary.dictionary_id())
            .cloned()
            .map_err(|_| CommandError::new("dictionary.not_found"))?;
        if request.snapshot_revision == 0 || request.snapshot_revision > dictionary.revision() {
            return Err(CommandError::new("ai.writeback_revision_invalid"));
        }
        let dictionary_snapshot = self.probe_dictionary_snapshot(dictionary.id())?;
        let mut observed_sources = BTreeSet::new();
        let mut page_number = 1;
        loop {
            let query = ProbeQuery::new(
                "",
                page_number,
                glyphshift_capture::MAX_PROBE_QUERY_PAGE_SIZE,
            )
            .map_err(probe::probe_run_error)?;
            let page = self
                .probe_runs
                .query_entries(&request.run_id, &query, &dictionary_snapshot)
                .map_err(probe::probe_run_error)?;
            for row in page.rows() {
                if row.state() != glyphshift_capture::ProbeEntryState::Unobserved {
                    observed_sources.insert(Box::<str>::from(row.source()));
                }
            }
            if observed_sources.len() >= page.total()
                || page.rows().len() < glyphshift_capture::MAX_PROBE_QUERY_PAGE_SIZE
            {
                break;
            }
            page_number = page_number.saturating_add(1);
        }

        let mut item_ids = BTreeSet::new();
        let mut result_sources = BTreeSet::new();
        let mut updates = Vec::new();
        let mut skipped_count = 0_usize;
        for result in request.results {
            let source = result.source.trim();
            let translation = result.translation.trim();
            if result.item_id.trim().is_empty()
                || source.is_empty()
                || translation.is_empty()
                || !item_ids.insert(result.item_id)
                || !result_sources.insert(Box::<str>::from(source))
                || !observed_sources.contains(source)
            {
                return Err(CommandError::new("ai.writeback_invalid"));
            }
            let already_completed = dictionary
                .entries()
                .iter()
                .any(|entry| entry.source() == source && !entry.translation().trim().is_empty());
            if already_completed {
                skipped_count = skipped_count.saturating_add(1);
            } else {
                updates.push(DictionaryEntryCreate::new(source, translation));
            }
        }
        let applied_count = updates.len();
        if !updates.is_empty() {
            self.backend
                .upsert_dictionary_entries(dictionary.id(), updates, dictionary.revision())
                .map_err(|_| CommandError::new("ai.writeback_conflict"))?;
            self.reconcile_enabled_workflows()?;
            self.publish_probe_preview_if_active(&request.run_id)?;
        }
        Ok(ProbeAiApplyView {
            probe: self.probe_run_summary(&request.run_id)?,
            applied_count,
            skipped_count,
        })
    }
}

#[cfg(windows)]
#[derive(Default)]
struct SystemCredentialVault;

#[cfg(windows)]
impl SystemCredentialVault {
    fn entry(credential_ref: &str) -> Result<keyring::Entry, CredentialVaultError> {
        keyring::Entry::new(AI_CREDENTIAL_SERVICE, credential_ref).map_err(map_keyring_error)
    }
}

#[cfg(windows)]
impl CredentialVault for SystemCredentialVault {
    fn replace(&self, credential_ref: &str, secret: &str) -> Result<(), CredentialVaultError> {
        Self::entry(credential_ref)?
            .set_password(secret)
            .map_err(map_keyring_error)
    }

    fn contains(&self, credential_ref: &str) -> Result<bool, CredentialVaultError> {
        match Self::entry(credential_ref)?.get_password() {
            Ok(_) => Ok(true),
            Err(keyring::Error::NoEntry) => Ok(false),
            Err(error) => Err(map_keyring_error(error)),
        }
    }

    fn delete(&self, credential_ref: &str) -> Result<(), CredentialVaultError> {
        match Self::entry(credential_ref)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(map_keyring_error(error)),
        }
    }

    fn expose(&self, credential_ref: &str) -> Result<Box<str>, CredentialVaultError> {
        Self::entry(credential_ref)?
            .get_password()
            .map(Into::into)
            .map_err(map_keyring_error)
    }
}

#[cfg(windows)]
fn map_keyring_error(error: keyring::Error) -> CredentialVaultError {
    match error {
        keyring::Error::NoEntry => CredentialVaultError::Missing,
        keyring::Error::Invalid(_, _) | keyring::Error::TooLong(_, _) => {
            CredentialVaultError::Rejected
        }
        _ => CredentialVaultError::Unavailable,
    }
}

#[cfg(not(windows))]
#[derive(Default)]
struct SystemCredentialVault;

#[cfg(not(windows))]
impl CredentialVault for SystemCredentialVault {
    fn replace(&self, _credential_ref: &str, _secret: &str) -> Result<(), CredentialVaultError> {
        Err(CredentialVaultError::Unavailable)
    }

    fn contains(&self, _credential_ref: &str) -> Result<bool, CredentialVaultError> {
        Ok(false)
    }

    fn delete(&self, _credential_ref: &str) -> Result<(), CredentialVaultError> {
        Ok(())
    }

    fn expose(&self, _credential_ref: &str) -> Result<Box<str>, CredentialVaultError> {
        Err(CredentialVaultError::Missing)
    }
}

fn system_credential_vault() -> Box<dyn CredentialVault> {
    Box::new(SystemCredentialVault)
}

fn ai_profile_error(error: AiProfileError) -> CommandError {
    match error {
        AiProfileError::Storage | AiProfileError::InvalidArtifact => {
            CommandError::new("ai.profile_storage_failed")
        }
        AiProfileError::InvalidProfile(field) => {
            CommandError::new("ai.profile_invalid").with_arg("field", field)
        }
        AiProfileError::UnknownProfile(profile_id) => {
            CommandError::new("ai.profile_not_found").with_arg("profileId", profile_id.to_string())
        }
        AiProfileError::Credential(CredentialVaultError::Missing) => {
            CommandError::new("ai.credential_missing")
        }
        AiProfileError::Credential(CredentialVaultError::Unavailable) => {
            CommandError::new("ai.credential_unavailable")
        }
        AiProfileError::Credential(CredentialVaultError::Rejected) => {
            CommandError::new("ai.credential_rejected")
        }
    }
}

fn ai_plan_error(error: PlanError) -> CommandError {
    match error {
        PlanError::InvalidExcludedPattern { index } => {
            CommandError::new("ai.filter_pattern_invalid")
                .with_arg("index", u64::try_from(index).unwrap_or(u64::MAX))
        }
    }
}

fn ai_job_error(error: TranslationJobError) -> CommandError {
    match error {
        TranslationJobError::UnknownPlan(plan_token) => {
            CommandError::new("ai.plan_not_found").with_arg("planToken", plan_token.to_string())
        }
        TranslationJobError::MissingProvider(protocol) => {
            CommandError::new("ai.provider_unavailable")
                .with_arg("protocol", format!("{protocol:?}"))
        }
        TranslationJobError::UnknownJob(job_id) => {
            CommandError::new("ai.job_not_found").with_arg("jobId", job_id.to_string())
        }
        TranslationJobError::StateUnavailable => CommandError::new("ai.job_state_unavailable"),
    }
}

fn ai_state_unavailable() -> CommandError {
    CommandError::new("ai.state_unavailable")
}

#[tauri::command]
pub(super) fn desktop_ai_profiles(
    state: State<'_, Mutex<DesktopAiState>>,
) -> Result<AiProfilesView, CommandError> {
    state
        .lock()
        .map_err(|_| ai_state_unavailable())?
        .profiles_view()
}

#[tauri::command]
pub(super) fn desktop_save_ai_profile(
    request: AiProfileSaveRequest,
    state: State<'_, Mutex<DesktopAiState>>,
) -> Result<AiProfilesView, CommandError> {
    state
        .lock()
        .map_err(|_| ai_state_unavailable())?
        .save_profile(request)
}

#[tauri::command]
pub(super) fn desktop_set_default_ai_profile(
    profile_id: String,
    state: State<'_, Mutex<DesktopAiState>>,
) -> Result<AiProfilesView, CommandError> {
    state
        .lock()
        .map_err(|_| ai_state_unavailable())?
        .set_default_profile(&profile_id)
}

#[tauri::command]
pub(super) fn desktop_delete_ai_profile(
    profile_id: String,
    state: State<'_, Mutex<DesktopAiState>>,
) -> Result<AiProfilesView, CommandError> {
    state
        .lock()
        .map_err(|_| ai_state_unavailable())?
        .delete_profile(&profile_id)
}

#[tauri::command]
pub(super) fn desktop_plan_ai_translation(
    request: AiTranslationPlanRequest,
    state: State<'_, Mutex<DesktopAiState>>,
) -> Result<TranslationPlan, CommandError> {
    state
        .lock()
        .map_err(|_| ai_state_unavailable())?
        .plan_translation(request)
}

#[tauri::command]
pub(super) fn desktop_plan_probe_ai_translation(
    request: ProbeAiPlanRequest,
    application: State<'_, Mutex<DesktopApplication>>,
    state: State<'_, Mutex<DesktopAiState>>,
) -> Result<TranslationPlan, CommandError> {
    let plan_request = application
        .lock()
        .map_err(|_| workspace_unavailable())?
        .probe_ai_plan_request(&request.run_id)?;
    state
        .lock()
        .map_err(|_| ai_state_unavailable())?
        .plan_request(request.profile_id.as_deref(), plan_request)
}

#[tauri::command]
pub(super) fn desktop_apply_probe_ai_results(
    request: ProbeAiApplyRequest,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<ProbeAiApplyView, CommandError> {
    application
        .lock()
        .map_err(|_| workspace_unavailable())?
        .apply_probe_ai_results(request)
}

#[tauri::command]
pub(super) fn desktop_start_ai_translation(
    request: AiTranslationStartRequest,
    state: State<'_, Mutex<DesktopAiState>>,
) -> Result<TranslationJobId, CommandError> {
    state
        .lock()
        .map_err(|_| ai_state_unavailable())?
        .start_translation(request)
}

#[tauri::command]
pub(super) fn desktop_ai_translation_job(
    job_id: String,
    state: State<'_, Mutex<DesktopAiState>>,
) -> Result<TranslationJobSnapshot, CommandError> {
    state
        .lock()
        .map_err(|_| ai_state_unavailable())?
        .translation_job(&job_id)
}

#[tauri::command]
pub(super) fn desktop_cancel_ai_translation(
    job_id: String,
    state: State<'_, Mutex<DesktopAiState>>,
) -> Result<CancellationOutcome, CommandError> {
    state
        .lock()
        .map_err(|_| ai_state_unavailable())?
        .cancel_translation(&job_id)
}

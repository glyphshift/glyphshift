use super::*;
use glyphshift_ai_translation::{
    AiProfileCatalog, AiProfileDraft, AiProfileError, AiProfileView, AiTranslation,
    CancellationOutcome, PlanError, ReqwestHttpTransport, TranslationItem, TranslationJobError,
    TranslationJobId, TranslationJobSnapshot, TranslationPlan, TranslationPlanRequest,
    TranslationRunHistory, TranslationRunRecord, ValidatedTranslation,
};
use std::path::Path;
use std::sync::Arc;

pub(super) struct DesktopAiState {
    profiles: AiProfileCatalog,
    translation: AiTranslation,
    history: TranslationRunHistory,
    active_task: Option<ActiveTranslationTask>,
}

#[derive(Clone)]
enum TranslationTaskOrigin {
    Dictionary,
    Probe { run_id: Box<str> },
    Connection,
}

#[derive(Clone)]
struct ActiveTranslationTask {
    job_id: TranslationJobId,
    target_dictionary_id: Box<str>,
    origin: TranslationTaskOrigin,
    processed_item_ids: BTreeSet<Box<str>>,
    applied_count: usize,
    skipped_count: usize,
    writeback_error: Option<Box<str>>,
    checkpoint_finished_batches: Option<usize>,
    writeback_attempted_results: usize,
    terminal_writeback_attempted: bool,
}

impl DesktopAiState {
    pub(super) fn open(data_root: &Path) -> Result<Self, String> {
        let profiles = AiProfileCatalog::open(data_root)
            .map_err(|error| format!("AI profile startup: {error:?}"))?;
        let history = TranslationRunHistory::open(data_root)
            .map_err(|error| format!("AI translation history startup: {error:?}"))?;
        let mut translation = AiTranslation::new();
        translation.register_first_release_http_providers(Arc::new(ReqwestHttpTransport));
        translation.register_codex_subscription_provider();
        Ok(Self {
            profiles,
            translation,
            history,
            active_task: None,
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

    fn start_translation_job(
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

    fn bind_task(
        &mut self,
        job_id: TranslationJobId,
        target_dictionary_id: impl Into<Box<str>>,
        origin: TranslationTaskOrigin,
    ) {
        self.active_task = Some(ActiveTranslationTask {
            job_id,
            target_dictionary_id: target_dictionary_id.into(),
            origin,
            processed_item_ids: BTreeSet::new(),
            applied_count: 0,
            skipped_count: 0,
            writeback_error: None,
            checkpoint_finished_batches: None,
            writeback_attempted_results: 0,
            terminal_writeback_attempted: false,
        });
    }

    fn translation_job(&mut self, job_id: &str) -> Result<TranslationJobSnapshot, CommandError> {
        let snapshot = self
            .translation
            .translation_job(&TranslationJobId::new(job_id))
            .map_err(ai_job_error)?;
        if snapshot.status().is_terminal() {
            let _ = self.history.record(&snapshot);
        }
        Ok(snapshot)
    }

    fn cancel_translation(&mut self, job_id: &str) -> Result<CancellationOutcome, CommandError> {
        let id = TranslationJobId::new(job_id);
        let outcome = self
            .translation
            .cancel_translation(&TranslationJobId::new(job_id))
            .map_err(ai_job_error)?;
        if let Ok(snapshot) = self.translation.translation_job(&id) {
            let _ = self.history.record(&snapshot);
        }
        Ok(outcome)
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(super) struct TranslationTaskView {
    #[serde(flatten)]
    job: TranslationJobSnapshot,
    target_dictionary_id: Box<str>,
    origin: Box<str>,
    applied_count: usize,
    skipped_count: usize,
    writeback_error: Option<Box<str>>,
    dictionary_locked: bool,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(super) struct TranslationTaskCenterView {
    current: Option<TranslationTaskView>,
    history: Vec<TranslationRunRecord>,
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
    fn resolve_ai_task_target(
        &mut self,
        snapshot: &TranslationJobSnapshot,
    ) -> Result<(Box<str>, TranslationTaskOrigin), CommandError> {
        if let Some(dictionary_id) = snapshot.scope_id().strip_prefix("dictionary:") {
            let dictionary = self
                .backend
                .dictionary(dictionary_id)
                .map_err(|_| CommandError::new("dictionary.not_found"))?;
            if dictionary.revision() != snapshot.snapshot_revision() {
                return Err(CommandError::new("ai.plan_stale"));
            }
            return Ok((dictionary_id.into(), TranslationTaskOrigin::Dictionary));
        }
        if let Some(run_id) = snapshot.scope_id().strip_prefix("probe:") {
            let summary = self
                .probe_runs
                .summary(run_id)
                .map_err(probe::probe_run_error)?;
            return Ok((
                summary.dictionary_id().into(),
                TranslationTaskOrigin::Probe {
                    run_id: run_id.into(),
                },
            ));
        }
        if snapshot.scope_id().starts_with("connection:") {
            return Ok((Box::<str>::from(""), TranslationTaskOrigin::Connection));
        }
        Err(CommandError::new("ai.task_scope_invalid"))
    }

    fn lock_ai_dictionary(&mut self, dictionary_id: &str) -> Result<(), CommandError> {
        if self
            .ai_locked_dictionary_id
            .as_deref()
            .is_some_and(|locked| locked != dictionary_id)
        {
            return Err(CommandError::new("ai.task_already_active"));
        }
        if !dictionary_id.is_empty() {
            self.ai_locked_dictionary_id = Some(dictionary_id.into());
        }
        Ok(())
    }

    fn unlock_ai_dictionary(&mut self, dictionary_id: &str) {
        if self.ai_locked_dictionary_id.as_deref() == Some(dictionary_id) {
            self.ai_locked_dictionary_id = None;
        }
    }

    pub(super) fn ensure_ai_dictionary_writable(
        &self,
        dictionary_id: &str,
    ) -> Result<(), CommandError> {
        if self.ai_locked_dictionary_id.as_deref() == Some(dictionary_id) {
            Err(CommandError::new("dictionary.ai_translation_locked"))
        } else {
            Ok(())
        }
    }

    fn apply_dictionary_ai_results(
        &mut self,
        dictionary_id: &str,
        results: &[ValidatedTranslation],
    ) -> Result<(usize, usize), CommandError> {
        let dictionary = self
            .backend
            .dictionary(dictionary_id)
            .cloned()
            .map_err(|_| CommandError::new("dictionary.not_found"))?;
        let mut item_ids = BTreeSet::new();
        let mut result_sources = BTreeSet::new();
        let mut updates = Vec::new();
        let mut skipped = 0_usize;
        for result in results {
            let source = result.source().trim();
            let translation = result.translation().trim();
            if result.item_id().trim().is_empty()
                || source.is_empty()
                || translation.is_empty()
                || !item_ids.insert(Box::<str>::from(result.item_id()))
                || !result_sources.insert(Box::<str>::from(source))
            {
                return Err(CommandError::new("ai.writeback_invalid"));
            }
            let Some(entry) = dictionary
                .entries()
                .iter()
                .find(|entry| entry.source() == source)
            else {
                skipped = skipped.saturating_add(1);
                continue;
            };
            if entry.translation().trim().is_empty() {
                updates.push(DictionaryEntryCreate::new(source, translation));
            } else {
                skipped = skipped.saturating_add(1);
            }
        }
        let applied = updates.len();
        if !updates.is_empty() {
            self.backend
                .upsert_dictionary_entries(dictionary.id(), updates, dictionary.revision())
                .map_err(|_| CommandError::new("ai.writeback_conflict"))?;
            self.reconcile_enabled_workflows()?;
        }
        Ok((applied, skipped))
    }

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
        let snapshot = self.probe_entries_snapshot(&summary)?;
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
                    if row.state() == glyphshift_capture::ProbeEntryState::Ignored || row.has_translation_conflict() {
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
        let dictionary_snapshot = self.probe_entries_snapshot(&summary)?;
        let mut observed_sources = BTreeSet::new();
        let mut protected_sources = BTreeSet::new();
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
                if row.has_translation_conflict() || !row.translation().trim().is_empty() { protected_sources.insert(Box::<str>::from(row.source())); }
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
            if already_completed || protected_sources.contains(source) {
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

fn sync_translation_task(
    application: &mut DesktopApplication,
    state: &mut DesktopAiState,
) -> Result<Option<TranslationTaskView>, CommandError> {
    let Some(active) = state.active_task.clone() else {
        return Ok(None);
    };
    let snapshot = state.translation_job(active.job_id.as_str())?;
    let pending = snapshot
        .results()
        .iter()
        .filter(|result| !active.processed_item_ids.contains(result.item_id()))
        .cloned()
        .collect::<Vec<_>>();
    let should_attempt_writeback = !pending.is_empty()
        && (active.writeback_error.is_none()
            || snapshot.results().len() > active.writeback_attempted_results
            || (snapshot.status().is_terminal() && !active.terminal_writeback_attempted));
    if should_attempt_writeback {
        let writeback = match &active.origin {
            TranslationTaskOrigin::Dictionary => {
                application.apply_dictionary_ai_results(&active.target_dictionary_id, &pending)
            }
            TranslationTaskOrigin::Probe { run_id } => application
                .apply_probe_ai_results(ProbeAiApplyRequest {
                    run_id: run_id.clone(),
                    snapshot_revision: snapshot.snapshot_revision(),
                    results: pending
                        .iter()
                        .map(|result| ProbeAiTranslationResult {
                            item_id: result.item_id().into(),
                            source: result.source().into(),
                            translation: result.translation().into(),
                        })
                        .collect(),
                })
                .map(|view| (view.applied_count, view.skipped_count)),
            TranslationTaskOrigin::Connection => Ok((pending.len(), 0)),
        };
        if let Some(current) = state.active_task.as_mut() {
            current.writeback_attempted_results = snapshot.results().len();
            current.terminal_writeback_attempted = snapshot.status().is_terminal();
            match writeback {
                Ok((applied, skipped)) => {
                    current.processed_item_ids.extend(
                        pending
                            .iter()
                            .map(|result| Box::<str>::from(result.item_id())),
                    );
                    current.applied_count = current.applied_count.saturating_add(applied);
                    current.skipped_count = current.skipped_count.saturating_add(skipped);
                    current.writeback_error = None;
                }
                Err(_) => current.writeback_error = Some("ai.writeback_failed".into()),
            }
        }
    }
    if snapshot.status().is_terminal() {
        if let Some(task) = state.active_task.as_ref() {
            let _ = state.history.record_with_writeback(
                &snapshot,
                task.applied_count,
                task.skipped_count,
            );
        }
        application.unlock_ai_dictionary(&active.target_dictionary_id);
    } else {
        let should_checkpoint = state.active_task.as_ref().is_some_and(|task| {
            task.checkpoint_finished_batches != Some(snapshot.finished_batches())
        });
        if should_checkpoint {
            let _ = state.history.checkpoint(&snapshot);
            if let Some(task) = state.active_task.as_mut() {
                task.checkpoint_finished_batches = Some(snapshot.finished_batches());
            }
        }
    }
    let current = state
        .active_task
        .as_ref()
        .expect("active task must remain visible");
    Ok(Some(TranslationTaskView {
        job: snapshot,
        target_dictionary_id: current.target_dictionary_id.clone(),
        origin: match &current.origin {
            TranslationTaskOrigin::Dictionary => "dictionary".into(),
            TranslationTaskOrigin::Probe { .. } => "probe".into(),
            TranslationTaskOrigin::Connection => "connection".into(),
        },
        applied_count: current.applied_count,
        skipped_count: current.skipped_count,
        writeback_error: current.writeback_error.clone(),
        dictionary_locked: application.ai_locked_dictionary_id.as_deref()
            == Some(current.target_dictionary_id.as_ref()),
    }))
}

fn monitor_translation_task(app: tauri::AppHandle, job_id: TranslationJobId) {
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_millis(120));
        let application_state = app.state::<Mutex<DesktopApplication>>();
        let ai_state = app.state::<Mutex<DesktopAiState>>();
        let Ok(mut application) = application_state.lock() else {
            return;
        };
        let Ok(mut state) = ai_state.lock() else {
            return;
        };
        if state
            .active_task
            .as_ref()
            .is_none_or(|active| active.job_id != job_id)
        {
            return;
        }
        match sync_translation_task(&mut application, &mut state) {
            Ok(Some(task)) if task.job.status().is_terminal() => return,
            Ok(Some(_)) => {}
            Ok(None) | Err(_) => return,
        }
    });
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
        AiProfileError::MissingCredential => CommandError::new("ai.credential_missing"),
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
        TranslationJobError::ActiveJob(job_id) => {
            CommandError::new("ai.task_already_active").with_arg("jobId", job_id.to_string())
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
    app: tauri::AppHandle,
    application: State<'_, Mutex<DesktopApplication>>,
    state: State<'_, Mutex<DesktopAiState>>,
) -> Result<TranslationTaskView, CommandError> {
    let mut application = application.lock().map_err(|_| workspace_unavailable())?;
    let mut state = state.lock().map_err(|_| ai_state_unavailable())?;
    let _ = sync_translation_task(&mut application, &mut state)?;
    let job_id = state.start_translation_job(request)?;
    let snapshot = state.translation_job(job_id.as_str())?;
    let (dictionary_id, origin) = match application.resolve_ai_task_target(&snapshot) {
        Ok(target) => target,
        Err(error) => {
            let _ = state.translation.cancel_translation(&job_id);
            return Err(error);
        }
    };
    if let Err(error) = application.lock_ai_dictionary(&dictionary_id) {
        let _ = state.translation.cancel_translation(&job_id);
        return Err(error);
    }
    state.bind_task(job_id.clone(), dictionary_id, origin);
    let task = sync_translation_task(&mut application, &mut state)?
        .ok_or_else(|| CommandError::new("ai.job_state_unavailable"))?;
    let should_monitor = !task.job.status().is_terminal();
    drop(state);
    drop(application);
    if should_monitor {
        monitor_translation_task(app, job_id);
    }
    Ok(task)
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
    application: State<'_, Mutex<DesktopApplication>>,
    state: State<'_, Mutex<DesktopAiState>>,
) -> Result<CancellationOutcome, CommandError> {
    let mut application = application.lock().map_err(|_| workspace_unavailable())?;
    let mut state = state.lock().map_err(|_| ai_state_unavailable())?;
    let outcome = state.cancel_translation(&job_id)?;
    let _ = sync_translation_task(&mut application, &mut state)?;
    Ok(outcome)
}

#[tauri::command]
pub(super) fn desktop_ai_translation_tasks(
    application: State<'_, Mutex<DesktopApplication>>,
    state: State<'_, Mutex<DesktopAiState>>,
) -> Result<TranslationTaskCenterView, CommandError> {
    let mut application = application.lock().map_err(|_| workspace_unavailable())?;
    let mut state = state.lock().map_err(|_| ai_state_unavailable())?;
    let current = sync_translation_task(&mut application, &mut state)?;
    Ok(TranslationTaskCenterView {
        current,
        history: state.history.records().to_vec(),
    })
}

use crate::{
    AiProviderProtocol, AiTranslation, ResolvedAiProfile, TranslationCandidate, TranslationPlan,
};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

pub const DEFAULT_MAX_ITEMS_PER_REQUEST: u16 = 50;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TranslationBatchPolicy {
    max_items_per_request: u16,
}

impl TranslationBatchPolicy {
    #[must_use]
    pub const fn new(max_items_per_request: u16) -> Option<Self> {
        if max_items_per_request == 0 {
            None
        } else {
            Some(Self {
                max_items_per_request,
            })
        }
    }

    #[must_use]
    pub const fn max_items_per_request(self) -> u16 {
        self.max_items_per_request
    }
}

impl Default for TranslationBatchPolicy {
    fn default() -> Self {
        Self {
            max_items_per_request: DEFAULT_MAX_ITEMS_PER_REQUEST,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderErrorCategory {
    Network,
    Timeout,
    Cancelled,
    Authentication,
    Permission,
    InvalidRequest,
    ModelNotFound,
    RateLimited,
    QuotaOrBilling,
    Overloaded,
    SafetyOrRefusal,
    MalformedOutput,
    ProviderInternal,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderError {
    category: ProviderErrorCategory,
    retryable: bool,
    retry_after_ms: Option<u64>,
    provider_code: Option<Box<str>>,
    request_id: Option<Box<str>>,
    http_status: Option<u16>,
    safe_message: Box<str>,
}

impl ProviderError {
    #[must_use]
    pub fn new(
        category: ProviderErrorCategory,
        retryable: bool,
        safe_message: impl Into<Box<str>>,
    ) -> Self {
        Self {
            category,
            retryable,
            retry_after_ms: None,
            provider_code: None,
            request_id: None,
            http_status: None,
            safe_message: safe_message.into(),
        }
    }

    #[must_use]
    pub const fn category(&self) -> ProviderErrorCategory {
        self.category
    }

    #[must_use]
    pub const fn retryable(&self) -> bool {
        self.retryable
    }

    #[must_use]
    pub const fn retry_after_ms(&self) -> Option<u64> {
        self.retry_after_ms
    }

    #[must_use]
    pub fn safe_message(&self) -> &str {
        &self.safe_message
    }

    #[must_use]
    pub(crate) fn with_http_status(mut self, status: u16) -> Self {
        self.http_status = Some(status);
        self
    }

    #[must_use]
    pub(crate) fn with_request_id(mut self, request_id: Option<&str>) -> Self {
        self.request_id = request_id.map(Into::into);
        self
    }

    #[must_use]
    pub(crate) fn with_retry_after_ms(mut self, retry_after_ms: Option<u64>) -> Self {
        self.retry_after_ms = retry_after_ms;
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderItem {
    item_id: Box<str>,
    source: Box<str>,
    protected_tokens: Vec<Box<str>>,
}

impl ProviderItem {
    #[must_use]
    pub fn item_id(&self) -> &str {
        &self.item_id
    }

    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    #[must_use]
    pub fn protected_tokens(&self) -> &[Box<str>] {
        &self.protected_tokens
    }
}

pub struct ProviderRequest<'a> {
    profile: &'a ResolvedAiProfile,
    source_locale: &'a str,
    target_locale: &'a str,
    items: Vec<ProviderItem>,
}

impl<'a> ProviderRequest<'a> {
    #[must_use]
    pub const fn profile(&self) -> &ResolvedAiProfile {
        self.profile
    }

    #[must_use]
    pub const fn source_locale(&self) -> &str {
        self.source_locale
    }

    #[must_use]
    pub const fn target_locale(&self) -> &str {
        self.target_locale
    }

    #[must_use]
    pub fn items(&self) -> &[ProviderItem] {
        &self.items
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderTranslation {
    item_id: Box<str>,
    text: Box<str>,
}

impl ProviderTranslation {
    #[must_use]
    pub fn new(item_id: impl Into<Box<str>>, text: impl Into<Box<str>>) -> Self {
        Self {
            item_id: item_id.into(),
            text: text.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderBatchResult {
    translations: Vec<ProviderTranslation>,
}

impl ProviderBatchResult {
    #[must_use]
    pub fn new(translations: impl IntoIterator<Item = ProviderTranslation>) -> Self {
        Self {
            translations: translations.into_iter().collect(),
        }
    }
}

#[derive(Clone)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    pub(crate) fn new(cancelled: Arc<AtomicBool>) -> Self {
        Self { cancelled }
    }

    #[cfg(test)]
    pub(crate) fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

pub trait TranslationProvider: Send + Sync + 'static {
    fn translate(
        &self,
        request: &ProviderRequest<'_>,
        cancellation: &CancellationToken,
    ) -> Result<ProviderBatchResult, ProviderError>;
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ValidatedTranslation {
    item_id: Box<str>,
    source: Box<str>,
    translation: Box<str>,
}

impl ValidatedTranslation {
    #[must_use]
    pub fn item_id(&self) -> &str {
        &self.item_id
    }

    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    #[must_use]
    pub fn translation(&self) -> &str {
        &self.translation
    }
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TranslationJobStatus {
    Queued,
    Running,
    Completed,
    CompletedWithFailures,
    Cancelling,
    Cancelled,
}

impl TranslationJobStatus {
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::CompletedWithFailures | Self::Cancelled
        )
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TranslationJobSnapshot {
    job_id: Box<str>,
    plan_token: Box<str>,
    scope_id: Box<str>,
    snapshot_revision: u64,
    status: TranslationJobStatus,
    total_count: usize,
    completed_count: usize,
    failed_count: usize,
    batch_size: u16,
    max_concurrency: u16,
    max_retries: u16,
    total_batches: usize,
    finished_batches: usize,
    failed_batches: usize,
    results: Vec<ValidatedTranslation>,
    errors: Vec<ProviderError>,
}

impl TranslationJobSnapshot {
    #[must_use]
    pub fn job_id(&self) -> &str {
        &self.job_id
    }

    #[must_use]
    pub const fn status(&self) -> TranslationJobStatus {
        self.status
    }

    #[must_use]
    pub const fn total_count(&self) -> usize {
        self.total_count
    }

    #[must_use]
    pub const fn completed_count(&self) -> usize {
        self.completed_count
    }

    #[must_use]
    pub const fn failed_count(&self) -> usize {
        self.failed_count
    }

    #[must_use]
    pub const fn batch_size(&self) -> u16 {
        self.batch_size
    }

    #[must_use]
    pub const fn max_concurrency(&self) -> u16 {
        self.max_concurrency
    }

    #[must_use]
    pub const fn max_retries(&self) -> u16 {
        self.max_retries
    }

    #[must_use]
    pub const fn total_batches(&self) -> usize {
        self.total_batches
    }

    #[must_use]
    pub const fn finished_batches(&self) -> usize {
        self.finished_batches
    }

    #[must_use]
    pub const fn failed_batches(&self) -> usize {
        self.failed_batches
    }

    #[must_use]
    pub fn results(&self) -> &[ValidatedTranslation] {
        &self.results
    }

    #[must_use]
    pub fn errors(&self) -> &[ProviderError] {
        &self.errors
    }
}

pub(crate) struct JobCell {
    state: Mutex<TranslationJobSnapshot>,
    cancelled: Arc<AtomicBool>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(transparent)]
pub struct TranslationJobId(Box<str>);

impl TranslationJobId {
    #[must_use]
    pub fn new(value: impl Into<Box<str>>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TranslationJobError {
    UnknownPlan(Box<str>),
    MissingProvider(AiProviderProtocol),
    UnknownJob(Box<str>),
    StateUnavailable,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CancellationOutcome {
    Requested,
    AlreadyFinished,
}

impl AiTranslation {
    pub fn register_provider(
        &mut self,
        protocol: AiProviderProtocol,
        provider: Arc<dyn TranslationProvider>,
    ) {
        self.providers.insert(protocol, provider);
    }

    pub fn start_translation(
        &mut self,
        plan_token: &str,
        profile: ResolvedAiProfile,
        batch_policy: TranslationBatchPolicy,
    ) -> Result<TranslationJobId, TranslationJobError> {
        let plan = self
            .plans
            .get(plan_token)
            .cloned()
            .ok_or_else(|| TranslationJobError::UnknownPlan(plan_token.into()))?;
        let provider = self
            .providers
            .get(&profile.protocol())
            .cloned()
            .ok_or(TranslationJobError::MissingProvider(profile.protocol()))?;
        self.next_job_id = self.next_job_id.saturating_add(1);
        let job_id: Box<str> = format!("job-{}", self.next_job_id).into();
        let cancelled = Arc::new(AtomicBool::new(false));
        let total_batches = batches(&plan.candidates, batch_policy).len();
        let cell = Arc::new(JobCell {
            state: Mutex::new(TranslationJobSnapshot {
                job_id: job_id.clone(),
                plan_token: plan.token.clone(),
                scope_id: plan.scope_id.clone(),
                snapshot_revision: plan.snapshot_revision,
                status: TranslationJobStatus::Queued,
                total_count: plan.candidates.len(),
                completed_count: 0,
                failed_count: 0,
                batch_size: batch_policy.max_items_per_request(),
                max_concurrency: profile.max_concurrency(),
                max_retries: profile.max_retries(),
                total_batches,
                finished_batches: 0,
                failed_batches: 0,
                results: Vec::new(),
                errors: Vec::new(),
            }),
            cancelled: cancelled.clone(),
        });
        self.jobs.insert(job_id.clone(), cell.clone());
        std::thread::spawn(move || run_job(cell, provider, profile, batch_policy, plan));
        Ok(TranslationJobId(job_id))
    }

    pub fn translation_job(
        &self,
        job_id: &TranslationJobId,
    ) -> Result<TranslationJobSnapshot, TranslationJobError> {
        let cell = self
            .jobs
            .get(job_id.as_str())
            .ok_or_else(|| TranslationJobError::UnknownJob(job_id.as_str().into()))?;
        cell.state
            .lock()
            .map(|state| state.clone())
            .map_err(|_| TranslationJobError::StateUnavailable)
    }

    pub fn cancel_translation(
        &self,
        job_id: &TranslationJobId,
    ) -> Result<CancellationOutcome, TranslationJobError> {
        let cell = self
            .jobs
            .get(job_id.as_str())
            .ok_or_else(|| TranslationJobError::UnknownJob(job_id.as_str().into()))?;
        let mut state = cell
            .state
            .lock()
            .map_err(|_| TranslationJobError::StateUnavailable)?;
        if state.status.is_terminal() {
            return Ok(CancellationOutcome::AlreadyFinished);
        }
        cell.cancelled.store(true, Ordering::Release);
        state.status = TranslationJobStatus::Cancelled;
        Ok(CancellationOutcome::Requested)
    }
}

fn run_job(
    cell: Arc<JobCell>,
    provider: Arc<dyn TranslationProvider>,
    profile: ResolvedAiProfile,
    batch_policy: TranslationBatchPolicy,
    plan: TranslationPlan,
) {
    if let Ok(mut state) = cell.state.lock() {
        if cell.cancelled.load(Ordering::Acquire) || state.status.is_terminal() {
            state.status = TranslationJobStatus::Cancelled;
            return;
        }
        state.status = TranslationJobStatus::Running;
    } else {
        return;
    }
    let token = CancellationToken::new(cell.cancelled.clone());
    let job_batches = batches(&plan.candidates, batch_policy);
    let worker_count = usize::from(profile.max_concurrency())
        .min(job_batches.len())
        .max(1);
    let next_batch = AtomicUsize::new(0);
    std::thread::scope(|scope| {
        for _ in 0..worker_count {
            let cell = &cell;
            let provider = provider.clone();
            let profile = &profile;
            let plan = &plan;
            let token = &token;
            let job_batches = &job_batches;
            let next_batch = &next_batch;
            scope.spawn(move || loop {
                if token.is_cancelled() {
                    return;
                }
                let batch_index = next_batch.fetch_add(1, Ordering::AcqRel);
                let Some(candidates) = job_batches.get(batch_index) else {
                    return;
                };
                let response = translate_batch(provider.as_ref(), profile, plan, candidates, token);
                if token.is_cancelled() {
                    return;
                }
                let Ok(mut state) = cell.state.lock() else {
                    return;
                };
                match response {
                    Ok(results) => {
                        state.completed_count += results.len();
                        state.finished_batches += 1;
                        state.results.extend(results);
                    }
                    Err(error) => {
                        state.failed_count += candidates.len();
                        state.finished_batches += 1;
                        state.failed_batches += 1;
                        state.errors.push(error);
                    }
                }
            });
        }
    });
    if token.is_cancelled() {
        finish_cancelled(&cell);
        return;
    }
    if let Ok(mut state) = cell.state.lock() {
        let result_order = plan
            .candidates
            .iter()
            .enumerate()
            .map(|(index, candidate)| (candidate.item_id.as_ref(), index))
            .collect::<BTreeMap<_, _>>();
        state.results.sort_by_key(|result| {
            result_order
                .get(result.item_id())
                .copied()
                .unwrap_or(usize::MAX)
        });
        state.status = if state.failed_count == 0 {
            TranslationJobStatus::Completed
        } else {
            TranslationJobStatus::CompletedWithFailures
        };
    }
}

fn translate_batch(
    provider: &dyn TranslationProvider,
    profile: &ResolvedAiProfile,
    plan: &TranslationPlan,
    candidates: &[TranslationCandidate],
    token: &CancellationToken,
) -> Result<Vec<ValidatedTranslation>, ProviderError> {
    let request = ProviderRequest {
        profile,
        source_locale: &plan.source_locale,
        target_locale: &plan.target_locale,
        items: candidates
            .iter()
            .map(|candidate| ProviderItem {
                item_id: candidate.item_id.clone(),
                source: candidate.source.clone(),
                protected_tokens: candidate.protected_tokens.clone(),
            })
            .collect(),
    };
    let mut attempt = 0_u32;
    let response = loop {
        let response = provider.translate(&request, token);
        match &response {
            Err(error)
                if error.retryable()
                    && attempt < u32::from(profile.max_retries())
                    && !token.is_cancelled() =>
            {
                let exponential = 250_u64.saturating_mul(1_u64 << attempt);
                let delay_ms = error.retry_after_ms().unwrap_or(exponential).min(5_000);
                attempt += 1;
                if wait_for_retry(delay_ms, token) {
                    break Err(ProviderError::new(
                        ProviderErrorCategory::Cancelled,
                        false,
                        "provider request was cancelled",
                    ));
                }
            }
            _ => break response,
        }
    };
    response.and_then(|response| validate_response(candidates, response))
}

fn finish_cancelled(cell: &JobCell) {
    if let Ok(mut state) = cell.state.lock() {
        state.status = TranslationJobStatus::Cancelled;
    }
}

fn batches(
    candidates: &[TranslationCandidate],
    policy: TranslationBatchPolicy,
) -> Vec<Vec<TranslationCandidate>> {
    candidates
        .chunks(usize::from(policy.max_items_per_request()))
        .map(<[TranslationCandidate]>::to_vec)
        .collect()
}

fn validate_response(
    candidates: &[TranslationCandidate],
    response: ProviderBatchResult,
) -> Result<Vec<ValidatedTranslation>, ProviderError> {
    let expected = candidates
        .iter()
        .map(|candidate| candidate.item_id.as_ref())
        .collect::<BTreeSet<_>>();
    let actual = response
        .translations
        .iter()
        .map(|translation| translation.item_id.as_ref())
        .collect::<BTreeSet<_>>();
    if response.translations.len() != candidates.len()
        || actual.len() != response.translations.len()
        || actual != expected
    {
        return Err(malformed_output("provider returned mismatched item ids"));
    }
    let by_id = response
        .translations
        .into_iter()
        .map(|translation| (translation.item_id.clone(), translation))
        .collect::<BTreeMap<_, _>>();
    candidates
        .iter()
        .map(|candidate| {
            let output = &by_id[&candidate.item_id];
            let text = output.text.trim();
            if text.is_empty()
                || candidate.protected_tokens.iter().any(|token| {
                    token_occurrences(&candidate.source, token) != token_occurrences(text, token)
                })
            {
                return Err(malformed_output(
                    "provider returned empty text or changed protected tokens",
                ));
            }
            Ok(ValidatedTranslation {
                item_id: candidate.item_id.clone(),
                source: candidate.source.clone(),
                translation: text.into(),
            })
        })
        .collect()
}

fn malformed_output(message: &'static str) -> ProviderError {
    ProviderError::new(ProviderErrorCategory::MalformedOutput, false, message)
}

fn token_occurrences(text: &str, token: &str) -> usize {
    text.match_indices(token).count()
}

fn wait_for_retry(delay_ms: u64, token: &CancellationToken) -> bool {
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(delay_ms);
    while std::time::Instant::now() < deadline {
        if token.is_cancelled() {
            return true;
        }
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        std::thread::sleep(remaining.min(std::time::Duration::from_millis(25)));
    }
    token.is_cancelled()
}

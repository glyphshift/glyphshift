use super::{ProviderError, ProviderUsage};
use crate::{AiProviderProtocol, AiReasoningEffort};
use serde::{Deserialize, Serialize};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ValidatedTranslation {
    pub(super) item_id: Box<str>,
    pub(super) source: Box<str>,
    pub(super) translation: Box<str>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) context: Option<Box<str>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) disambiguation: Option<Box<str>>,
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

    #[must_use]
    pub fn context(&self) -> Option<&str> {
        self.context.as_deref()
    }

    #[must_use]
    pub fn disambiguation(&self) -> Option<&str> {
        self.disambiguation.as_deref()
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TranslationJobStatus {
    Queued,
    Running,
    Completed,
    CompletedWithFailures,
    Cancelling,
    Cancelled,
    Interrupted,
}

impl TranslationJobStatus {
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::CompletedWithFailures | Self::Cancelled | Self::Interrupted
        )
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TranslationBatchStatus {
    Queued,
    Running,
    Retrying,
    Completed,
    Failed,
    Cancelled,
}

impl TranslationBatchStatus {
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TranslationBatchSnapshot {
    pub(super) batch_number: usize,
    pub(super) item_count: usize,
    pub(super) status: TranslationBatchStatus,
    pub(super) attempt_count: u16,
    pub(super) started_after_ms: Option<u64>,
    pub(super) elapsed_ms: u64,
    pub(super) last_error: Option<ProviderError>,
    pub(super) usage: Option<ProviderUsage>,
}

impl TranslationBatchSnapshot {
    #[must_use]
    pub const fn batch_number(&self) -> usize {
        self.batch_number
    }

    #[must_use]
    pub const fn status(&self) -> TranslationBatchStatus {
        self.status
    }

    #[must_use]
    pub const fn item_count(&self) -> usize {
        self.item_count
    }

    #[must_use]
    pub const fn attempt_count(&self) -> u16 {
        self.attempt_count
    }

    #[must_use]
    pub const fn usage(&self) -> Option<ProviderUsage> {
        self.usage
    }

    #[must_use]
    pub const fn elapsed_ms(&self) -> u64 {
        self.elapsed_ms
    }

    #[must_use]
    pub const fn last_error(&self) -> Option<&ProviderError> {
        self.last_error.as_ref()
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TranslationJobSnapshot {
    pub(super) job_id: Box<str>,
    pub(super) plan_token: Box<str>,
    pub(super) scope_id: Box<str>,
    pub(super) snapshot_revision: u64,
    pub(super) started_at_ms: u64,
    pub(super) finished_at_ms: Option<u64>,
    pub(super) profile_name: Box<str>,
    pub(super) protocol: AiProviderProtocol,
    pub(super) model_id: Box<str>,
    pub(super) reasoning_effort: AiReasoningEffort,
    pub(super) status: TranslationJobStatus,
    pub(super) total_count: usize,
    pub(super) completed_count: usize,
    pub(super) failed_count: usize,
    pub(super) batch_size: u16,
    pub(super) max_concurrency: u16,
    pub(super) max_retries: u16,
    pub(super) total_batches: usize,
    pub(super) finished_batches: usize,
    pub(super) failed_batches: usize,
    pub(super) elapsed_ms: u64,
    pub(super) peak_concurrency: usize,
    pub(super) usage: Option<ProviderUsage>,
    pub(super) batches: Vec<TranslationBatchSnapshot>,
    pub(super) results: Vec<ValidatedTranslation>,
    pub(super) errors: Vec<ProviderError>,
}

impl TranslationJobSnapshot {
    #[must_use]
    pub fn job_id(&self) -> &str {
        &self.job_id
    }

    #[must_use]
    pub fn scope_id(&self) -> &str {
        &self.scope_id
    }

    #[must_use]
    pub const fn snapshot_revision(&self) -> u64 {
        self.snapshot_revision
    }

    #[must_use]
    pub const fn started_at_ms(&self) -> u64 {
        self.started_at_ms
    }

    #[must_use]
    pub const fn finished_at_ms(&self) -> Option<u64> {
        self.finished_at_ms
    }

    #[must_use]
    pub fn profile_name(&self) -> &str {
        &self.profile_name
    }

    #[must_use]
    pub const fn protocol(&self) -> AiProviderProtocol {
        self.protocol
    }

    #[must_use]
    pub fn model_id(&self) -> &str {
        &self.model_id
    }

    #[must_use]
    pub const fn reasoning_effort(&self) -> AiReasoningEffort {
        self.reasoning_effort
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
    pub const fn peak_concurrency(&self) -> usize {
        self.peak_concurrency
    }

    #[must_use]
    pub const fn elapsed_ms(&self) -> u64 {
        self.elapsed_ms
    }

    #[must_use]
    pub const fn usage(&self) -> Option<ProviderUsage> {
        self.usage
    }

    #[must_use]
    pub fn batches(&self) -> &[TranslationBatchSnapshot] {
        &self.batches
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
    pub(super) state: Mutex<JobState>,
    pub(super) cancelled: Arc<AtomicBool>,
}

pub(super) struct JobState {
    pub(super) snapshot: TranslationJobSnapshot,
    pub(super) started_at: Instant,
}

impl JobState {
    pub(super) fn elapsed_ms(&self) -> u64 {
        u64::try_from(self.started_at.elapsed().as_millis()).unwrap_or(u64::MAX)
    }

    pub(super) fn visible_snapshot(&self) -> TranslationJobSnapshot {
        let mut snapshot = self.snapshot.clone();
        let elapsed_ms = if snapshot.status.is_terminal() {
            snapshot.elapsed_ms
        } else {
            self.elapsed_ms()
        };
        snapshot.elapsed_ms = elapsed_ms;
        for batch in &mut snapshot.batches {
            if !batch.status.is_terminal() {
                if let Some(started_after_ms) = batch.started_after_ms {
                    batch.elapsed_ms = elapsed_ms.saturating_sub(started_after_ms);
                }
            }
        }
        snapshot
    }
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
    ActiveJob(Box<str>),
    UnknownJob(Box<str>),
    StateUnavailable,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CancellationOutcome {
    Requested,
    AlreadyFinished,
}

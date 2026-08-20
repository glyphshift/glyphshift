use crate::{
    AiProviderProtocol, AiReasoningEffort, ProviderError, ProviderUsage, TranslationBatchSnapshot,
    TranslationBatchStatus, TranslationJobSnapshot, TranslationJobStatus,
};
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tempfile::NamedTempFile;

const HISTORY_SCHEMA: &str = "glyphshift.ai-translation-history/2";
const HISTORY_FILE_NAME: &str = "ai-translation-history.json";
const MAX_HISTORY_RECORDS: usize = 100;

const fn default_historical_reasoning_effort() -> AiReasoningEffort {
    AiReasoningEffort::Automatic
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TranslationRunBatch {
    batch_number: usize,
    item_count: usize,
    status: TranslationBatchStatus,
    attempt_count: u16,
    elapsed_ms: u64,
    last_error: Option<ProviderError>,
    usage: Option<ProviderUsage>,
}

impl From<&TranslationBatchSnapshot> for TranslationRunBatch {
    fn from(batch: &TranslationBatchSnapshot) -> Self {
        Self {
            batch_number: batch.batch_number(),
            item_count: batch.item_count(),
            status: batch.status(),
            attempt_count: batch.attempt_count(),
            elapsed_ms: batch.elapsed_ms(),
            last_error: batch.last_error().cloned(),
            usage: batch.usage(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TranslationRunRecord {
    record_id: Box<str>,
    started_at_ms: u64,
    finished_at_ms: u64,
    scope_kind: Box<str>,
    profile_name: Box<str>,
    protocol: AiProviderProtocol,
    model_id: Box<str>,
    #[serde(default = "default_historical_reasoning_effort")]
    reasoning_effort: AiReasoningEffort,
    status: TranslationJobStatus,
    total_count: usize,
    completed_count: usize,
    failed_count: usize,
    #[serde(default)]
    applied_count: usize,
    #[serde(default)]
    skipped_count: usize,
    total_batches: usize,
    finished_batches: usize,
    failed_batches: usize,
    request_attempts: usize,
    retry_attempts: usize,
    elapsed_ms: u64,
    peak_concurrency: usize,
    usage: Option<ProviderUsage>,
    batches: Vec<TranslationRunBatch>,
}

impl TranslationRunRecord {
    fn from_snapshot(snapshot: &TranslationJobSnapshot) -> Self {
        let batches = snapshot
            .batches()
            .iter()
            .map(TranslationRunBatch::from)
            .collect::<Vec<_>>();
        let request_attempts = batches
            .iter()
            .map(|batch| usize::from(batch.attempt_count))
            .sum();
        let retry_attempts = batches
            .iter()
            .map(|batch| usize::from(batch.attempt_count.saturating_sub(1)))
            .sum();
        let scope_kind = snapshot
            .scope_id()
            .split_once(':')
            .map_or(snapshot.scope_id(), |(kind, _)| kind);
        Self {
            record_id: format!("{}-{}", snapshot.started_at_ms(), snapshot.job_id()).into(),
            started_at_ms: snapshot.started_at_ms(),
            finished_at_ms: snapshot.finished_at_ms().unwrap_or_else(|| {
                snapshot
                    .started_at_ms()
                    .saturating_add(snapshot.elapsed_ms())
            }),
            scope_kind: scope_kind.into(),
            profile_name: snapshot.profile_name().into(),
            protocol: snapshot.protocol(),
            model_id: snapshot.model_id().into(),
            reasoning_effort: snapshot.reasoning_effort(),
            status: snapshot.status(),
            total_count: snapshot.total_count(),
            completed_count: snapshot.completed_count(),
            failed_count: snapshot.failed_count(),
            applied_count: snapshot.completed_count(),
            skipped_count: 0,
            total_batches: snapshot.total_batches(),
            finished_batches: snapshot.finished_batches(),
            failed_batches: snapshot.failed_batches(),
            request_attempts,
            retry_attempts,
            elapsed_ms: snapshot.elapsed_ms(),
            peak_concurrency: snapshot.peak_concurrency(),
            usage: snapshot.usage(),
            batches,
        }
    }

    #[must_use]
    pub fn scope_kind(&self) -> &str {
        &self.scope_kind
    }

    #[must_use]
    pub fn profile_name(&self) -> &str {
        &self.profile_name
    }

    #[must_use]
    pub fn model_id(&self) -> &str {
        &self.model_id
    }

    #[must_use]
    pub const fn status(&self) -> TranslationJobStatus {
        self.status
    }

    #[must_use]
    pub const fn reasoning_effort(&self) -> AiReasoningEffort {
        self.reasoning_effort
    }

    #[must_use]
    pub const fn total_count(&self) -> usize {
        self.total_count
    }

    #[must_use]
    pub const fn applied_count(&self) -> usize {
        self.applied_count
    }

    #[must_use]
    pub const fn skipped_count(&self) -> usize {
        self.skipped_count
    }

    #[must_use]
    pub const fn request_attempts(&self) -> usize {
        self.request_attempts
    }

    #[must_use]
    pub const fn usage(&self) -> Option<ProviderUsage> {
        self.usage
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct HistoryArtifact {
    schema: Box<str>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    active: Option<TranslationRunRecord>,
    records: Vec<TranslationRunRecord>,
}

impl Default for HistoryArtifact {
    fn default() -> Self {
        Self {
            schema: HISTORY_SCHEMA.into(),
            active: None,
            records: Vec::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TranslationRunHistoryError {
    InvalidArtifact,
    Storage,
}

pub struct TranslationRunHistory {
    path: PathBuf,
    artifact: HistoryArtifact,
}

impl TranslationRunHistory {
    pub fn open(data_root: impl AsRef<Path>) -> Result<Self, TranslationRunHistoryError> {
        let path = data_root.as_ref().join(HISTORY_FILE_NAME);
        let artifact = if path.exists() {
            let reader =
                BufReader::new(File::open(&path).map_err(|_| TranslationRunHistoryError::Storage)?);
            let value = match serde_json::from_reader::<_, serde_json::Value>(reader) {
                Ok(value) => value,
                Err(_) => {
                    preserve_invalid_history_artifact(&path);
                    serde_json::Value::Null
                }
            };
            history_artifact_from_value(&value)
        } else {
            HistoryArtifact::default()
        };
        let mut history = Self { path, artifact };
        if let Some(mut interrupted) = history.artifact.active.take() {
            interrupted.status = TranslationJobStatus::Interrupted;
            interrupted.finished_at_ms = unix_time_ms();
            history
                .artifact
                .records
                .retain(|candidate| candidate.record_id != interrupted.record_id);
            history.artifact.records.insert(0, interrupted);
            history.artifact.records.truncate(MAX_HISTORY_RECORDS);
            history.persist()?;
        }
        Ok(history)
    }

    #[must_use]
    pub fn records(&self) -> &[TranslationRunRecord] {
        &self.artifact.records
    }

    pub fn record(
        &mut self,
        snapshot: &TranslationJobSnapshot,
    ) -> Result<(), TranslationRunHistoryError> {
        self.record_with_writeback(snapshot, snapshot.completed_count(), 0)
    }

    pub fn record_with_writeback(
        &mut self,
        snapshot: &TranslationJobSnapshot,
        applied_count: usize,
        skipped_count: usize,
    ) -> Result<(), TranslationRunHistoryError> {
        if !snapshot.status().is_terminal() {
            return Ok(());
        }
        let mut record = TranslationRunRecord::from_snapshot(snapshot);
        record.applied_count = applied_count;
        record.skipped_count = skipped_count;
        self.artifact.active = None;
        self.artifact
            .records
            .retain(|candidate| candidate.record_id != record.record_id);
        self.artifact.records.insert(0, record);
        self.artifact.records.truncate(MAX_HISTORY_RECORDS);
        self.persist()
    }

    pub fn checkpoint(
        &mut self,
        snapshot: &TranslationJobSnapshot,
    ) -> Result<(), TranslationRunHistoryError> {
        if snapshot.status().is_terminal() {
            return self.record(snapshot);
        }
        self.artifact.active = Some(TranslationRunRecord::from_snapshot(snapshot));
        self.persist()
    }

    pub fn clear(&mut self) -> Result<(), TranslationRunHistoryError> {
        self.artifact.records.clear();
        self.artifact.active = None;
        self.persist()
    }

    fn persist(&self) -> Result<(), TranslationRunHistoryError> {
        let parent = self
            .path
            .parent()
            .ok_or(TranslationRunHistoryError::Storage)?;
        fs::create_dir_all(parent).map_err(|_| TranslationRunHistoryError::Storage)?;
        let mut temporary =
            NamedTempFile::new_in(parent).map_err(|_| TranslationRunHistoryError::Storage)?;
        {
            let mut writer = BufWriter::new(temporary.as_file_mut());
            serde_json::to_writer_pretty(&mut writer, &self.artifact)
                .map_err(|_| TranslationRunHistoryError::Storage)?;
            writer
                .write_all(b"\n")
                .map_err(|_| TranslationRunHistoryError::Storage)?;
            writer
                .flush()
                .map_err(|_| TranslationRunHistoryError::Storage)?;
        }
        temporary
            .as_file()
            .sync_all()
            .map_err(|_| TranslationRunHistoryError::Storage)?;
        temporary
            .persist(&self.path)
            .map_err(|_| TranslationRunHistoryError::Storage)?;
        Ok(())
    }
}

fn history_artifact_from_value(value: &serde_json::Value) -> HistoryArtifact {
    let Some(object) = value.as_object() else {
        return HistoryArtifact::default();
    };
    let schema = object
        .get("schema")
        .and_then(serde_json::Value::as_str)
        .filter(|schema| *schema == HISTORY_SCHEMA)
        .unwrap_or(HISTORY_SCHEMA);
    let active = object
        .get("active")
        .and_then(|record| serde_json::from_value(record.clone()).ok());
    let mut records = object
        .get("records")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|record| serde_json::from_value(record.clone()).ok())
        .collect::<Vec<_>>();
    records.truncate(MAX_HISTORY_RECORDS);
    HistoryArtifact {
        schema: schema.into(),
        active,
        records,
    }
}

fn preserve_invalid_history_artifact(path: &Path) {
    let backup = path.with_extension("invalid.json");
    if !backup.exists() {
        let _ = fs::copy(path, backup);
    }
}

fn unix_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
        })
}

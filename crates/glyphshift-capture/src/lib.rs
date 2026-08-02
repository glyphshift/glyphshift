//! Bounded text capture, technical provenance catalog, and pure dictionary drafts.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{sync_channel, SyncSender, TrySendError};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{SystemTime, UNIX_EPOCH};

pub const CAPTURE_CATALOG_SCHEMA: &str = "glyphshift.capture-catalog/1";
pub const DICTIONARY_DRAFT_SCHEMA: &str = "glyphshift.dictionary-draft/1";
pub const DEFAULT_MAX_ENTRIES: u32 = 50_000;
const MAX_ENTRIES: u32 = 250_000;
const MAX_SOURCE_UNITS: usize = 16 * 1024;
const QUEUE_CAPACITY: usize = 4_096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptureError {
    InvalidConfiguration,
    InvalidCatalog,
    Storage,
    WorkerUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CaptureSessionId(Box<str>);

impl CaptureSessionId {
    pub fn new(value: impl Into<Box<str>>) -> Result<Self, CaptureError> {
        let value = value.into();
        if safe_identifier(&value) {
            Ok(Self(value))
        } else {
            Err(CaptureError::InvalidConfiguration)
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaptureConfiguration {
    session_id: CaptureSessionId,
    output_path: PathBuf,
    max_entries: u32,
}

impl CaptureConfiguration {
    pub fn new(
        session_id: CaptureSessionId,
        output_path: impl Into<PathBuf>,
        max_entries: u32,
    ) -> Result<Self, CaptureError> {
        let output_path = output_path.into();
        if !output_path.is_absolute() || max_entries == 0 || max_entries > MAX_ENTRIES {
            return Err(CaptureError::InvalidConfiguration);
        }
        Ok(Self {
            session_id,
            output_path,
            max_entries,
        })
    }

    #[must_use]
    pub const fn session_id(&self) -> &CaptureSessionId {
        &self.session_id
    }

    #[must_use]
    pub fn output_path(&self) -> &Path {
        &self.output_path
    }

    #[must_use]
    pub const fn max_entries(&self) -> u32 {
        self.max_entries
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CaptureCatalogEntry {
    source: Box<str>,
    adapter_id: Box<str>,
    count: u64,
    first_seen_ms: u64,
    last_seen_ms: u64,
}

impl CaptureCatalogEntry {
    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    #[must_use]
    pub fn adapter_id(&self) -> &str {
        &self.adapter_id
    }

    #[must_use]
    pub const fn count(&self) -> u64 {
        self.count
    }

    #[must_use]
    pub const fn first_seen_ms(&self) -> u64 {
        self.first_seen_ms
    }

    #[must_use]
    pub const fn last_seen_ms(&self) -> u64 {
        self.last_seen_ms
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CaptureCatalog {
    schema: Box<str>,
    session_id: CaptureSessionId,
    started_at_ms: u64,
    stopped_at_ms: u64,
    dropped_observations: u64,
    entries: Vec<CaptureCatalogEntry>,
}

impl CaptureCatalog {
    #[must_use]
    pub const fn session_id(&self) -> &CaptureSessionId {
        &self.session_id
    }

    #[must_use]
    pub const fn started_at_ms(&self) -> u64 {
        self.started_at_ms
    }

    #[must_use]
    pub const fn stopped_at_ms(&self) -> u64 {
        self.stopped_at_ms
    }

    #[must_use]
    pub const fn dropped_observations(&self) -> u64 {
        self.dropped_observations
    }

    #[must_use]
    pub fn entries(&self) -> &[CaptureCatalogEntry] {
        &self.entries
    }

    pub fn decode_json(source: &str) -> Result<Self, CaptureError> {
        let catalog: Self =
            serde_json::from_str(source).map_err(|_| CaptureError::InvalidCatalog)?;
        catalog.validate()?;
        Ok(catalog)
    }

    pub fn encode_json(&self) -> Result<String, CaptureError> {
        self.validate()?;
        serde_json::to_string(self).map_err(|_| CaptureError::Storage)
    }

    pub fn read(path: &Path) -> Result<Self, CaptureError> {
        let source = fs::read_to_string(path).map_err(|_| CaptureError::Storage)?;
        Self::decode_json(&source)
    }

    #[must_use]
    pub fn dictionary_draft(&self) -> DictionaryDraft {
        let entries = self
            .entries
            .iter()
            .map(|entry| entry.source.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(|source| DictionaryDraftEntry {
                source,
                translation: "".into(),
            })
            .collect();
        DictionaryDraft {
            schema: DICTIONARY_DRAFT_SCHEMA.into(),
            source_session_id: self.session_id.clone(),
            entries,
        }
    }

    fn validate(&self) -> Result<(), CaptureError> {
        let valid_entries = self.entries.iter().all(|entry| {
            !entry.source.trim().is_empty()
                && entry.source.encode_utf16().count() <= MAX_SOURCE_UNITS
                && safe_identifier(&entry.adapter_id)
                && entry.count > 0
                && entry.first_seen_ms <= entry.last_seen_ms
        });
        let unique_entries = self
            .entries
            .iter()
            .map(|entry| (&entry.source, &entry.adapter_id))
            .collect::<BTreeSet<_>>()
            .len()
            == self.entries.len();
        if self.schema.as_ref() != CAPTURE_CATALOG_SCHEMA
            || self.stopped_at_ms < self.started_at_ms
            || self.entries.len() > MAX_ENTRIES as usize
            || !valid_entries
            || !unique_entries
        {
            return Err(CaptureError::InvalidCatalog);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct DictionaryDraftEntry {
    source: Box<str>,
    translation: Box<str>,
}

impl DictionaryDraftEntry {
    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    #[must_use]
    pub fn translation(&self) -> &str {
        &self.translation
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct DictionaryDraft {
    schema: Box<str>,
    source_session_id: CaptureSessionId,
    entries: Vec<DictionaryDraftEntry>,
}

impl DictionaryDraft {
    #[must_use]
    pub const fn source_session_id(&self) -> &CaptureSessionId {
        &self.source_session_id
    }

    #[must_use]
    pub fn entries(&self) -> &[DictionaryDraftEntry] {
        &self.entries
    }

    pub fn encode_json(&self) -> Result<String, CaptureError> {
        serde_json::to_string(self).map_err(|_| CaptureError::Storage)
    }
}

struct CaptureCatalogBuilder {
    session_id: CaptureSessionId,
    started_at_ms: u64,
    max_entries: usize,
    entries: BTreeMap<(Box<str>, Box<str>), CaptureCatalogEntry>,
}

impl CaptureCatalogBuilder {
    fn record(&mut self, adapter_id: Box<str>, source: &str, observed_at_ms: u64) -> bool {
        let source = source.trim();
        if source.is_empty()
            || source.encode_utf16().count() > MAX_SOURCE_UNITS
            || !safe_identifier(&adapter_id)
        {
            return false;
        }
        let key = (source.into(), adapter_id.clone());
        if let Some(entry) = self.entries.get_mut(&key) {
            entry.count = entry.count.saturating_add(1);
            entry.last_seen_ms = entry.last_seen_ms.max(observed_at_ms);
            entry.first_seen_ms = entry.first_seen_ms.min(observed_at_ms);
            return true;
        }
        if self.entries.len() >= self.max_entries {
            return false;
        }
        self.entries.insert(
            key,
            CaptureCatalogEntry {
                source: source.into(),
                adapter_id,
                count: 1,
                first_seen_ms: observed_at_ms,
                last_seen_ms: observed_at_ms,
            },
        );
        true
    }

    fn finish(self, stopped_at_ms: u64, dropped_observations: u64) -> CaptureCatalog {
        CaptureCatalog {
            schema: CAPTURE_CATALOG_SCHEMA.into(),
            session_id: self.session_id,
            started_at_ms: self.started_at_ms,
            stopped_at_ms,
            dropped_observations,
            entries: self.entries.into_values().collect(),
        }
    }
}

enum CaptureCommand {
    Observe {
        adapter_id: Box<str>,
        source: Box<str>,
        observed_at_ms: u64,
    },
    Finish,
}

pub struct FileCaptureSink {
    sender: SyncSender<CaptureCommand>,
    dropped: Arc<AtomicU64>,
    worker: Option<JoinHandle<Result<CaptureCatalog, CaptureError>>>,
}

impl FileCaptureSink {
    pub fn start(configuration: CaptureConfiguration) -> Result<Self, CaptureError> {
        let parent = configuration
            .output_path()
            .parent()
            .ok_or(CaptureError::InvalidConfiguration)?;
        fs::create_dir_all(parent).map_err(|_| CaptureError::Storage)?;
        let (sender, receiver) = sync_channel(QUEUE_CAPACITY);
        let dropped = Arc::new(AtomicU64::new(0));
        let worker_dropped = dropped.clone();
        let worker = thread::Builder::new()
            .name("glyphshift-capture".into())
            .spawn(move || {
                let mut builder = CaptureCatalogBuilder {
                    session_id: configuration.session_id().clone(),
                    started_at_ms: unix_time_millis(),
                    max_entries: configuration.max_entries() as usize,
                    entries: BTreeMap::new(),
                };
                while let Ok(command) = receiver.recv() {
                    match command {
                        CaptureCommand::Observe {
                            adapter_id,
                            source,
                            observed_at_ms,
                        } => {
                            if !builder.record(adapter_id, &source, observed_at_ms) {
                                worker_dropped.fetch_add(1, Ordering::Relaxed);
                            }
                        }
                        CaptureCommand::Finish => break,
                    }
                }
                let catalog =
                    builder.finish(unix_time_millis(), worker_dropped.load(Ordering::Relaxed));
                write_catalog_atomic(configuration.output_path(), &catalog)?;
                Ok(catalog)
            })
            .map_err(|_| CaptureError::WorkerUnavailable)?;
        Ok(Self {
            sender,
            dropped,
            worker: Some(worker),
        })
    }

    pub fn observe(&self, adapter_id: impl Into<Box<str>>, source: impl Into<Box<str>>) {
        let command = CaptureCommand::Observe {
            adapter_id: adapter_id.into(),
            source: source.into(),
            observed_at_ms: unix_time_millis(),
        };
        if matches!(self.sender.try_send(command), Err(TrySendError::Full(_))) {
            self.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn finish(mut self) -> Result<CaptureCatalog, CaptureError> {
        self.sender
            .send(CaptureCommand::Finish)
            .map_err(|_| CaptureError::WorkerUnavailable)?;
        self.worker
            .take()
            .ok_or(CaptureError::WorkerUnavailable)?
            .join()
            .map_err(|_| CaptureError::WorkerUnavailable)?
    }
}

fn write_catalog_atomic(path: &Path, catalog: &CaptureCatalog) -> Result<(), CaptureError> {
    let pending = path.with_extension("pending");
    fs::write(&pending, catalog.encode_json()?).map_err(|_| CaptureError::Storage)?;
    fs::rename(pending, path).map_err(|_| CaptureError::Storage)
}

#[must_use]
pub fn unix_time_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis() as u64)
}

fn safe_identifier(value: &str) -> bool {
    !value.is_empty()
        && value != "."
        && value != ".."
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_')
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn capture_sink_deduplicates_by_source_and_adapter_then_builds_a_pure_draft() {
        let root = tempdir().expect("capture root");
        let output = root.path().join("capture.json");
        let sink = FileCaptureSink::start(
            CaptureConfiguration::new(
                CaptureSessionId::new("capture-1").expect("session id"),
                &output,
                10,
            )
            .expect("configuration"),
        )
        .expect("capture sink");
        sink.observe("windows.gdi.text-out", "Open");
        sink.observe("windows.gdi.text-out", "Open");
        sink.observe("windows.user32.draw-text", "Open");
        sink.observe("windows.user32.draw-text", "Close");

        let catalog = sink.finish().expect("finished catalog");
        assert_eq!(catalog.entries().len(), 3);
        assert_eq!(
            catalog
                .entries()
                .iter()
                .find(|entry| {
                    entry.source() == "Open" && entry.adapter_id() == "windows.gdi.text-out"
                })
                .map(CaptureCatalogEntry::count),
            Some(2),
        );
        assert_eq!(
            CaptureCatalog::read(&output).expect("saved catalog"),
            catalog
        );
        let draft = catalog.dictionary_draft();
        assert_eq!(draft.entries().len(), 2);
        assert!(draft
            .entries()
            .iter()
            .all(|entry| entry.translation().is_empty()));
        let encoded = draft.encode_json().expect("draft json");
        assert!(!encoded.contains("adapterId"));
        assert!(!encoded.contains("technology"));
    }

    #[test]
    fn capture_sink_caps_unique_entries_without_blocking_the_observer() {
        let root = tempdir().expect("capture root");
        let output = root.path().join("capture.json");
        let sink = FileCaptureSink::start(
            CaptureConfiguration::new(
                CaptureSessionId::new("capture-limit").expect("session id"),
                &output,
                1,
            )
            .expect("configuration"),
        )
        .expect("capture sink");
        sink.observe("windows.gdi.text-out", "One");
        sink.observe("windows.gdi.text-out", "Two");
        let catalog = sink.finish().expect("finished catalog");

        assert_eq!(catalog.entries().len(), 1);
        assert_eq!(catalog.dropped_observations(), 1);
    }
}

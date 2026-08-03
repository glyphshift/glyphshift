//! Bounded text observations and resumable probe runs.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{sync_channel, RecvTimeoutError, SyncSender, TrySendError};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

mod workspace;

pub use workspace::*;

pub const CAPTURE_CATALOG_SCHEMA: &str = "glyphshift.capture-catalog/2";
pub const DEFAULT_MAX_ENTRIES: u32 = 50_000;
const MAX_ENTRIES: u32 = 250_000;
const MAX_SOURCE_UNITS: usize = 16 * 1024;
const QUEUE_CAPACITY: usize = 8_192;
const CHECKPOINT_INTERVAL: Duration = Duration::from_secs(1);

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
    revision: u64,
    started_at_ms: u64,
    updated_at_ms: u64,
    dropped_observations: u64,
    entries: Vec<CaptureCatalogEntry>,
}

impl CaptureCatalog {
    #[must_use]
    pub const fn session_id(&self) -> &CaptureSessionId {
        &self.session_id
    }

    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    #[must_use]
    pub const fn started_at_ms(&self) -> u64 {
        self.started_at_ms
    }

    #[must_use]
    pub const fn updated_at_ms(&self) -> u64 {
        self.updated_at_ms
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

    pub fn read_current(path: &Path) -> Result<Self, CaptureError> {
        checkpoint_paths(path)
            .iter()
            .filter_map(|candidate| Self::read(candidate).ok())
            .max_by_key(Self::revision)
            .ok_or(CaptureError::Storage)
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
            || self.updated_at_ms < self.started_at_ms
            || self.entries.len() > MAX_ENTRIES as usize
            || !valid_entries
            || !unique_entries
        {
            return Err(CaptureError::InvalidCatalog);
        }
        Ok(())
    }
}

struct CaptureCatalogBuilder {
    session_id: CaptureSessionId,
    started_at_ms: u64,
    max_entries: usize,
    revision: u64,
    entries: BTreeMap<(Box<str>, Box<str>), CaptureCatalogEntry>,
}

impl CaptureCatalogBuilder {
    fn resume(configuration: &CaptureConfiguration) -> Result<(Self, u64), CaptureError> {
        let now = unix_time_millis();
        let Ok(previous) = CaptureCatalog::read_current(configuration.output_path()) else {
            return Ok((
                Self {
                    session_id: configuration.session_id().clone(),
                    started_at_ms: now,
                    max_entries: configuration.max_entries() as usize,
                    revision: 0,
                    entries: BTreeMap::new(),
                },
                0,
            ));
        };
        let revision = previous.revision();
        if previous.session_id() != configuration.session_id() {
            return Ok((
                Self {
                    session_id: configuration.session_id().clone(),
                    started_at_ms: now,
                    max_entries: configuration.max_entries() as usize,
                    revision,
                    entries: BTreeMap::new(),
                },
                0,
            ));
        }
        if previous.entries.len() > configuration.max_entries() as usize {
            return Err(CaptureError::InvalidConfiguration);
        }
        let dropped_observations = previous.dropped_observations();
        let started_at_ms = previous.started_at_ms();
        let entries = previous
            .entries
            .into_iter()
            .map(|entry| ((entry.source.clone(), entry.adapter_id.clone()), entry))
            .collect();
        Ok((
            Self {
                session_id: configuration.session_id().clone(),
                started_at_ms,
                max_entries: configuration.max_entries() as usize,
                revision,
                entries,
            },
            dropped_observations,
        ))
    }

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

    fn snapshot(&mut self, updated_at_ms: u64, dropped_observations: u64) -> CaptureCatalog {
        self.revision = self.revision.saturating_add(1);
        CaptureCatalog {
            schema: CAPTURE_CATALOG_SCHEMA.into(),
            session_id: self.session_id.clone(),
            revision: self.revision,
            started_at_ms: self.started_at_ms,
            updated_at_ms,
            dropped_observations,
            entries: self.entries.values().cloned().collect(),
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
    paused: Arc<AtomicBool>,
    worker: Option<JoinHandle<Result<CaptureCatalog, CaptureError>>>,
}

impl FileCaptureSink {
    pub fn start(configuration: CaptureConfiguration) -> Result<Self, CaptureError> {
        let parent = configuration
            .output_path()
            .parent()
            .ok_or(CaptureError::InvalidConfiguration)?;
        fs::create_dir_all(parent).map_err(|_| CaptureError::Storage)?;
        let (mut builder, initial_dropped) = CaptureCatalogBuilder::resume(&configuration)?;
        let (sender, receiver) = sync_channel(QUEUE_CAPACITY);
        let dropped = Arc::new(AtomicU64::new(initial_dropped));
        let paused = Arc::new(AtomicBool::new(false));
        let worker_dropped = dropped.clone();
        let worker = thread::Builder::new()
            .name("glyphshift-capture".into())
            .spawn(move || {
                let mut dirty = true;
                let mut finishing = false;
                while !finishing {
                    let command = match receiver.recv_timeout(CHECKPOINT_INTERVAL) {
                        Ok(command) => Some(command),
                        Err(RecvTimeoutError::Timeout) => None,
                        Err(RecvTimeoutError::Disconnected) => {
                            finishing = true;
                            None
                        }
                    };
                    let checkpoint_due = command.is_none();
                    match command {
                        Some(CaptureCommand::Observe {
                            adapter_id,
                            source,
                            observed_at_ms,
                        }) => {
                            if !builder.record(adapter_id, &source, observed_at_ms) {
                                worker_dropped.fetch_add(1, Ordering::Relaxed);
                            }
                            dirty = true;
                        }
                        Some(CaptureCommand::Finish) => finishing = true,
                        None => {}
                    }
                    if dirty && (checkpoint_due || finishing) {
                        let catalog = builder
                            .snapshot(unix_time_millis(), worker_dropped.load(Ordering::Relaxed));
                        write_catalog_checkpoint(configuration.output_path(), &catalog)?;
                        dirty = false;
                    }
                }
                let catalog =
                    builder.snapshot(unix_time_millis(), worker_dropped.load(Ordering::Relaxed));
                write_catalog_checkpoint(configuration.output_path(), &catalog)?;
                Ok(catalog)
            })
            .map_err(|_| CaptureError::WorkerUnavailable)?;
        Ok(Self {
            sender,
            dropped,
            paused,
            worker: Some(worker),
        })
    }

    pub fn observe(&self, adapter_id: impl Into<Box<str>>, source: impl Into<Box<str>>) {
        if self.paused.load(Ordering::Relaxed) {
            return;
        }
        let command = CaptureCommand::Observe {
            adapter_id: adapter_id.into(),
            source: source.into(),
            observed_at_ms: unix_time_millis(),
        };
        if matches!(self.sender.try_send(command), Err(TrySendError::Full(_))) {
            self.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn set_paused(&self, paused: bool) {
        self.paused.store(paused, Ordering::Relaxed);
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

fn write_catalog_checkpoint(path: &Path, catalog: &CaptureCatalog) -> Result<(), CaptureError> {
    let slots = checkpoint_paths(path);
    let destination = &slots[(catalog.revision() % 2) as usize];
    let pending = destination.with_extension("pending");
    fs::write(&pending, catalog.encode_json()?).map_err(|_| CaptureError::Storage)?;
    if destination.exists() {
        fs::remove_file(destination).map_err(|_| CaptureError::Storage)?;
    }
    fs::rename(pending, destination).map_err(|_| CaptureError::Storage)
}

fn checkpoint_paths(path: &Path) -> [PathBuf; 2] {
    [path.with_extension("a.json"), path.with_extension("b.json")]
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
    fn capture_sink_deduplicates_observations_by_source_and_adapter() {
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
            CaptureCatalog::read_current(&output).expect("saved catalog"),
            catalog
        );
        let encoded = catalog.encode_json().expect("catalog json");
        assert!(encoded.contains("adapterId"));
        assert!(!encoded.contains("translation"));
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

    #[test]
    fn capture_sink_checkpoints_while_running_and_pause_does_not_end_the_session() {
        let root = tempdir().expect("capture root");
        let output = root.path().join("capture.json");
        let sink = FileCaptureSink::start(
            CaptureConfiguration::new(
                CaptureSessionId::new("capture-live").expect("session id"),
                &output,
                10,
            )
            .expect("configuration"),
        )
        .expect("capture sink");
        sink.observe("windows.gdi.text-out", "Before pause");
        std::thread::sleep(Duration::from_millis(1_100));
        let live = CaptureCatalog::read_current(&output).expect("live checkpoint");
        assert_eq!(live.entries().len(), 1);

        sink.set_paused(true);
        sink.observe("windows.gdi.text-out", "Ignored while paused");
        sink.set_paused(false);
        sink.observe("windows.gdi.text-out", "After resume");
        let finished = sink.finish().expect("finish capture");
        assert_eq!(finished.entries().len(), 2);
        assert!(finished
            .entries()
            .iter()
            .all(|entry| entry.source() != "Ignored while paused"));
    }

    #[test]
    fn capture_sink_resumes_an_existing_catalog_without_resetting_its_revision_or_entries() {
        let root = tempdir().expect("capture root");
        let output = root.path().join("capture.json");
        let configuration = || {
            CaptureConfiguration::new(
                CaptureSessionId::new("capture-resume").expect("session id"),
                &output,
                10,
            )
            .expect("configuration")
        };

        let first = FileCaptureSink::start(configuration()).expect("first capture sink");
        first.observe("windows.gdi.text-out", "Before reconnect");
        let first_catalog = first.finish().expect("first catalog");

        let resumed = FileCaptureSink::start(configuration()).expect("resumed capture sink");
        resumed.observe("windows.gdi.text-out", "After reconnect");
        let resumed_catalog = resumed.finish().expect("resumed catalog");

        assert!(resumed_catalog.revision() > first_catalog.revision());
        assert!(resumed_catalog
            .entries()
            .iter()
            .any(|entry| entry.source() == "Before reconnect"));
        assert!(resumed_catalog
            .entries()
            .iter()
            .any(|entry| entry.source() == "After reconnect"));
        assert_eq!(
            CaptureCatalog::read_current(&output).expect("current catalog"),
            resumed_catalog
        );
    }
}

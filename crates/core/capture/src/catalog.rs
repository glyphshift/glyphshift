use crate::observation::{safe_identifier, MAX_ENTRIES, MAX_SOURCE_UNITS};
use crate::{CaptureConfiguration, CaptureError, CaptureIngressStatus, CaptureSessionId};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{sync_channel, RecvTimeoutError, SyncSender, TrySendError};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub const CAPTURE_CATALOG_SCHEMA: &str = "glyphshift.capture-catalog/2";
pub const DEFAULT_MAX_ENTRIES: u32 = 50_000;
const QUEUE_CAPACITY: usize = 8_192;
const CHECKPOINT_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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
#[serde(rename_all = "camelCase")]
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
    fallback_adapters: BTreeSet<Box<str>>,
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
                    fallback_adapters: configuration.fallback_adapters().clone(),
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
                    fallback_adapters: configuration.fallback_adapters().clone(),
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
                fallback_adapters: configuration.fallback_adapters().clone(),
            },
            dropped_observations,
        ))
    }

    fn record(&mut self, adapter_id: Box<str>, source: &str, observed_at_ms: u64) -> u64 {
        let source = source.trim();
        if source.is_empty()
            || source.encode_utf16().count() > MAX_SOURCE_UNITS
            || !safe_identifier(&adapter_id)
        {
            return 1;
        }
        let key = (source.into(), adapter_id.clone());
        if let Some(entry) = self.entries.get_mut(&key) {
            entry.count = entry.count.saturating_add(1);
            entry.last_seen_ms = entry.last_seen_ms.max(observed_at_ms);
            entry.first_seen_ms = entry.first_seen_ms.min(observed_at_ms);
            return 0;
        }
        if self.entries.len() >= self.max_entries {
            if self.fallback_adapters.contains(&adapter_id) {
                return 1;
            }
            let fallback_key = self
                .entries
                .iter()
                .filter(|((_, existing_adapter_id), _)| {
                    self.fallback_adapters.contains(existing_adapter_id)
                })
                .min_by(|(left_key, left), (right_key, right)| {
                    left.last_seen_ms
                        .cmp(&right.last_seen_ms)
                        .then_with(|| left_key.cmp(right_key))
                })
                .map(|(key, _)| key.clone());
            let Some(fallback_key) = fallback_key else {
                return 1;
            };
            let displaced = self
                .entries
                .remove(&fallback_key)
                .map_or(0, |entry| entry.count);
            self.insert(adapter_id, source, observed_at_ms);
            return displaced;
        }
        self.insert(adapter_id, source, observed_at_ms);
        0
    }

    fn insert(&mut self, adapter_id: Box<str>, source: &str, observed_at_ms: u64) {
        let key = (source.into(), adapter_id.clone());
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

/// Cloneable, non-blocking input for a single capture owner.
///
/// Producers can live on different threads, but they never own the checkpoint
/// file or its revision. The [`FileCaptureSink`] that created the ingress
/// remains the only writer and controls pause, checkpoint, and finish.
#[derive(Clone)]
pub struct CaptureIngress {
    sender: SyncSender<CaptureCommand>,
    dropped: Arc<AtomicU64>,
    paused: Arc<AtomicBool>,
    accepting: Arc<AtomicBool>,
    in_flight: Arc<AtomicU64>,
}

impl CaptureIngress {
    #[must_use]
    pub fn try_observe(
        &self,
        adapter_id: impl Into<Box<str>>,
        source: impl Into<Box<str>>,
    ) -> CaptureIngressStatus {
        if !self.accepting.load(Ordering::Acquire) {
            self.dropped.fetch_add(1, Ordering::Relaxed);
            return CaptureIngressStatus::Dropped;
        }
        if self.paused.load(Ordering::Relaxed) {
            return CaptureIngressStatus::Paused;
        }
        let command = CaptureCommand::Observe {
            adapter_id: adapter_id.into(),
            source: source.into(),
            observed_at_ms: unix_time_millis(),
        };
        self.in_flight.fetch_add(1, Ordering::AcqRel);
        if !self.accepting.load(Ordering::Acquire) {
            self.in_flight.fetch_sub(1, Ordering::AcqRel);
            self.dropped.fetch_add(1, Ordering::Relaxed);
            return CaptureIngressStatus::Dropped;
        }
        let status = match self.sender.try_send(command) {
            Ok(()) => CaptureIngressStatus::Accepted,
            Err(TrySendError::Full(_) | TrySendError::Disconnected(_)) => {
                self.dropped.fetch_add(1, Ordering::Relaxed);
                CaptureIngressStatus::Dropped
            }
        };
        self.in_flight.fetch_sub(1, Ordering::AcqRel);
        status
    }

    #[must_use]
    pub fn dropped_observations(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }

    /// Adds drops reported by an upstream supervised producer.
    pub fn report_dropped(&self, count: u64) {
        let _ = self
            .dropped
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                Some(current.saturating_add(count))
            });
    }
}

pub struct FileCaptureSink {
    ingress: CaptureIngress,
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
        let accepting = Arc::new(AtomicBool::new(true));
        let in_flight = Arc::new(AtomicU64::new(0));
        let worker_dropped = dropped.clone();
        let worker = thread::Builder::new()
            .name("glyphshift-capture".into())
            .spawn(move || {
                let mut dirty = true;
                let mut finishing = false;
                let mut checkpoint_deadline = Instant::now() + CHECKPOINT_INTERVAL;
                while !finishing {
                    let command = match receiver
                        .recv_timeout(checkpoint_deadline.saturating_duration_since(Instant::now()))
                    {
                        Ok(command) => Some(command),
                        Err(RecvTimeoutError::Timeout) => None,
                        Err(RecvTimeoutError::Disconnected) => {
                            finishing = true;
                            None
                        }
                    };
                    let checkpoint_due = Instant::now() >= checkpoint_deadline;
                    match command {
                        Some(CaptureCommand::Observe {
                            adapter_id,
                            source,
                            observed_at_ms,
                        }) => {
                            let dropped = builder.record(adapter_id, &source, observed_at_ms);
                            if dropped > 0 {
                                worker_dropped.fetch_add(dropped, Ordering::Relaxed);
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
                    if checkpoint_due {
                        checkpoint_deadline = Instant::now() + CHECKPOINT_INTERVAL;
                    }
                }
                let catalog =
                    builder.snapshot(unix_time_millis(), worker_dropped.load(Ordering::Relaxed));
                write_catalog_checkpoint(configuration.output_path(), &catalog)?;
                Ok(catalog)
            })
            .map_err(|_| CaptureError::WorkerUnavailable)?;
        Ok(Self {
            ingress: CaptureIngress {
                sender,
                dropped,
                paused: paused.clone(),
                accepting,
                in_flight,
            },
            paused,
            worker: Some(worker),
        })
    }

    #[must_use]
    pub fn ingress(&self) -> CaptureIngress {
        self.ingress.clone()
    }

    pub fn observe(&self, adapter_id: impl Into<Box<str>>, source: impl Into<Box<str>>) {
        let _ = self.ingress.try_observe(adapter_id, source);
    }

    pub fn set_paused(&self, paused: bool) {
        self.paused.store(paused, Ordering::Relaxed);
    }

    pub fn finish(mut self) -> Result<CaptureCatalog, CaptureError> {
        self.close_ingress();
        self.ingress
            .sender
            .send(CaptureCommand::Finish)
            .map_err(|_| CaptureError::WorkerUnavailable)?;
        self.worker
            .take()
            .ok_or(CaptureError::WorkerUnavailable)?
            .join()
            .map_err(|_| CaptureError::WorkerUnavailable)?
    }

    fn close_ingress(&self) {
        self.ingress.accepting.store(false, Ordering::Release);
        while self.ingress.in_flight.load(Ordering::Acquire) != 0 {
            std::thread::yield_now();
        }
    }
}

impl Drop for FileCaptureSink {
    fn drop(&mut self) {
        if self.worker.is_some() {
            self.close_ingress();
            let _ = self.ingress.sender.send(CaptureCommand::Finish);
        }
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

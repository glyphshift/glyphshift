//! Bounded text observations and resumable probe runs.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::sync::{Arc, RwLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

mod workspace;

pub use workspace::*;

pub const CAPTURE_CATALOG_SCHEMA: &str = "glyphshift.capture-catalog/2";
pub const CAPTURE_OBSERVATION_BATCH_SCHEMA: &str = "glyphshift.capture-observation-batch/1";
pub const DEFAULT_MAX_ENTRIES: u32 = 50_000;
const MAX_ENTRIES: u32 = 250_000;
const MAX_SOURCE_UNITS: usize = 16 * 1024;
const MAX_PRODUCER_ID_BYTES: usize = 256;
const MAX_OBSERVATION_BATCH_RECORDS: usize = 256;
pub const MAX_OBSERVATION_BATCH_BYTES: usize = 4 * 1024 * 1024;
const QUEUE_CAPACITY: usize = 8_192;
const CHECKPOINT_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptureError {
    InvalidConfiguration,
    InvalidCatalog,
    InvalidObservationBatch,
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

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CaptureProducerId(Box<str>);

impl CaptureProducerId {
    pub fn new(value: impl Into<Box<str>>) -> Result<Self, CaptureError> {
        let value = value.into();
        if value.len() <= MAX_PRODUCER_ID_BYTES && safe_identifier(&value) {
            Ok(Self(value))
        } else {
            Err(CaptureError::InvalidObservationBatch)
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaptureProducerConfiguration {
    producer_id: CaptureProducerId,
    generation: u64,
}

impl CaptureProducerConfiguration {
    pub fn new(producer_id: CaptureProducerId, generation: u64) -> Result<Self, CaptureError> {
        if generation == 0 {
            return Err(CaptureError::InvalidObservationBatch);
        }
        Ok(Self {
            producer_id,
            generation,
        })
    }

    #[must_use]
    pub const fn producer_id(&self) -> &CaptureProducerId {
        &self.producer_id
    }

    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CaptureObservationRecord {
    sequence: u64,
    adapter_id: Box<str>,
    source: Box<str>,
}

impl CaptureObservationRecord {
    pub fn new(
        sequence: u64,
        adapter_id: impl Into<Box<str>>,
        source: impl Into<Box<str>>,
    ) -> Result<Self, CaptureError> {
        let record = Self {
            sequence,
            adapter_id: adapter_id.into(),
            source: source.into(),
        };
        record.validate()?;
        Ok(record)
    }

    #[must_use]
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    #[must_use]
    pub fn adapter_id(&self) -> &str {
        &self.adapter_id
    }

    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    fn validate(&self) -> Result<(), CaptureError> {
        if self.sequence == 0
            || !safe_identifier(&self.adapter_id)
            || self.source.trim().is_empty()
            || self.source.encode_utf16().count() > MAX_SOURCE_UNITS
        {
            return Err(CaptureError::InvalidObservationBatch);
        }
        Ok(())
    }
}

/// Versioned, bounded handoff from one supervised observation producer.
///
/// `generation` is assigned by the producer supervisor and changes whenever
/// the producer restarts. `sequence` is local to that generation and is
/// allocated before the producer's bounded queue, so consumers can diagnose
/// gaps. `dropped_total` is the cumulative producer-side drop count for the
/// generation, not an instruction to mutate a checkpoint directly.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CaptureObservationBatch {
    schema: Box<str>,
    producer_id: CaptureProducerId,
    generation: u64,
    dropped_total: u64,
    records: Vec<CaptureObservationRecord>,
}

impl CaptureObservationBatch {
    pub fn new(
        producer_id: CaptureProducerId,
        generation: u64,
        dropped_total: u64,
        records: impl IntoIterator<Item = CaptureObservationRecord>,
    ) -> Result<Self, CaptureError> {
        let batch = Self {
            schema: CAPTURE_OBSERVATION_BATCH_SCHEMA.into(),
            producer_id,
            generation,
            dropped_total,
            records: records.into_iter().collect(),
        };
        batch.validate()?;
        Ok(batch)
    }

    #[must_use]
    pub const fn producer_id(&self) -> &CaptureProducerId {
        &self.producer_id
    }

    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    #[must_use]
    pub const fn dropped_total(&self) -> u64 {
        self.dropped_total
    }

    #[must_use]
    pub fn records(&self) -> &[CaptureObservationRecord] {
        &self.records
    }

    pub fn encode_json(&self) -> Result<String, CaptureError> {
        self.validate()?;
        let encoded =
            serde_json::to_string(self).map_err(|_| CaptureError::InvalidObservationBatch)?;
        if encoded.len() > MAX_OBSERVATION_BATCH_BYTES {
            return Err(CaptureError::InvalidObservationBatch);
        }
        Ok(encoded)
    }

    pub fn decode_json(source: &str) -> Result<Self, CaptureError> {
        if source.len() > MAX_OBSERVATION_BATCH_BYTES {
            return Err(CaptureError::InvalidObservationBatch);
        }
        let batch: Self =
            serde_json::from_str(source).map_err(|_| CaptureError::InvalidObservationBatch)?;
        batch.validate()?;
        Ok(batch)
    }

    fn validate(&self) -> Result<(), CaptureError> {
        let records_valid = self.records.len() <= MAX_OBSERVATION_BATCH_RECORDS
            && self.records.iter().all(|record| record.validate().is_ok())
            && self
                .records
                .windows(2)
                .all(|records| records[0].sequence() < records[1].sequence());
        if self.schema.as_ref() != CAPTURE_OBSERVATION_BATCH_SCHEMA
            || self.generation == 0
            || !records_valid
        {
            return Err(CaptureError::InvalidObservationBatch);
        }
        Ok(())
    }
}

/// Stateful validator for one supervised producer generation.
///
/// Individual batches validate their own bounds. This cursor additionally
/// rejects producer swaps, generation reuse, replayed records, and sequence
/// gaps that are not explained by the producer's cumulative drop counter.
pub struct CaptureObservationCursor {
    producer_id: CaptureProducerId,
    generation: u64,
    last_sequence: u64,
    dropped_total: u64,
}

impl CaptureObservationCursor {
    #[must_use]
    pub fn new(configuration: &CaptureProducerConfiguration) -> Self {
        Self {
            producer_id: configuration.producer_id().clone(),
            generation: configuration.generation(),
            last_sequence: 0,
            dropped_total: 0,
        }
    }

    /// Accepts the next batch and returns newly reported producer-side drops.
    pub fn accept(&mut self, batch: &CaptureObservationBatch) -> Result<u64, CaptureError> {
        if batch.producer_id() != &self.producer_id || batch.generation() != self.generation {
            return Err(CaptureError::InvalidObservationBatch);
        }
        let dropped_delta = batch
            .dropped_total()
            .checked_sub(self.dropped_total)
            .ok_or(CaptureError::InvalidObservationBatch)?;
        let next_sequence = batch
            .records()
            .first()
            .map_or(self.last_sequence, CaptureObservationRecord::sequence);
        if next_sequence <= self.last_sequence && !batch.records().is_empty() {
            return Err(CaptureError::InvalidObservationBatch);
        }
        let last_sequence = batch
            .records()
            .last()
            .map_or(self.last_sequence, CaptureObservationRecord::sequence);
        let sequence_span = last_sequence
            .checked_sub(self.last_sequence)
            .ok_or(CaptureError::InvalidObservationBatch)?;
        let sequence_gaps = sequence_span
            .checked_sub(batch.records().len() as u64)
            .ok_or(CaptureError::InvalidObservationBatch)?;
        if dropped_delta < sequence_gaps {
            return Err(CaptureError::InvalidObservationBatch);
        }
        self.last_sequence = last_sequence;
        self.dropped_total = batch.dropped_total();
        Ok(dropped_delta)
    }
}

/// Hot-path input owned by one [`CaptureBatchProducer`].
///
/// The read lock is acquired with `try_read`, so a concurrent drain never
/// blocks the observed application thread. Contention is reported through the
/// cumulative producer drop count.
#[derive(Clone)]
pub struct CaptureBatchIngress {
    sender: SyncSender<CaptureObservationRecord>,
    sequence: Arc<AtomicU64>,
    dropped: Arc<AtomicU64>,
    paused: Arc<AtomicBool>,
    accepting: Arc<AtomicBool>,
    drain_gate: Arc<RwLock<()>>,
}

impl CaptureBatchIngress {
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
        let Ok(_permit) = self.drain_gate.try_read() else {
            self.dropped.fetch_add(1, Ordering::Relaxed);
            return CaptureIngressStatus::Dropped;
        };
        if !self.accepting.load(Ordering::Acquire) {
            self.dropped.fetch_add(1, Ordering::Relaxed);
            return CaptureIngressStatus::Dropped;
        }
        let sequence = self.sequence.fetch_add(1, Ordering::Relaxed);
        let Ok(record) = CaptureObservationRecord::new(sequence, adapter_id, source) else {
            self.dropped.fetch_add(1, Ordering::Relaxed);
            return CaptureIngressStatus::Dropped;
        };
        match self.sender.try_send(record) {
            Ok(()) => CaptureIngressStatus::Accepted,
            Err(TrySendError::Full(_) | TrySendError::Disconnected(_)) => {
                self.dropped.fetch_add(1, Ordering::Relaxed);
                CaptureIngressStatus::Dropped
            }
        }
    }

    #[must_use]
    pub fn dropped_total(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }
}

/// Bounded observation producer drained by an out-of-process transport.
pub struct CaptureBatchProducer {
    producer_id: CaptureProducerId,
    generation: u64,
    receiver: Receiver<CaptureObservationRecord>,
    pending: VecDeque<CaptureObservationRecord>,
    dropped: Arc<AtomicU64>,
    paused: Arc<AtomicBool>,
    accepting: Arc<AtomicBool>,
    drain_gate: Arc<RwLock<()>>,
}

impl CaptureBatchProducer {
    pub fn start(
        configuration: CaptureProducerConfiguration,
    ) -> Result<(Self, CaptureBatchIngress), CaptureError> {
        let (sender, receiver) = sync_channel(QUEUE_CAPACITY);
        let sequence = Arc::new(AtomicU64::new(1));
        let dropped = Arc::new(AtomicU64::new(0));
        let paused = Arc::new(AtomicBool::new(false));
        let accepting = Arc::new(AtomicBool::new(true));
        let drain_gate = Arc::new(RwLock::new(()));
        let ingress = CaptureBatchIngress {
            sender,
            sequence,
            dropped: dropped.clone(),
            paused: paused.clone(),
            accepting: accepting.clone(),
            drain_gate: drain_gate.clone(),
        };
        Ok((
            Self {
                producer_id: configuration.producer_id,
                generation: configuration.generation,
                receiver,
                pending: VecDeque::new(),
                dropped,
                paused,
                accepting,
                drain_gate,
            },
            ingress,
        ))
    }

    pub fn drain(&mut self) -> Result<CaptureObservationBatch, CaptureError> {
        let _permit = self
            .drain_gate
            .write()
            .map_err(|_| CaptureError::WorkerUnavailable)?;
        let mut received = self.receiver.try_iter().collect::<Vec<_>>();
        received.sort_by_key(CaptureObservationRecord::sequence);
        self.pending.extend(received);
        let records = (0..MAX_OBSERVATION_BATCH_RECORDS)
            .filter_map(|_| self.pending.pop_front())
            .collect::<Vec<_>>();
        CaptureObservationBatch::new(
            self.producer_id.clone(),
            self.generation,
            self.dropped.load(Ordering::Relaxed),
            records,
        )
    }

    pub fn set_paused(&self, paused: bool) {
        self.paused.store(paused, Ordering::Relaxed);
    }
}

impl Drop for CaptureBatchProducer {
    fn drop(&mut self) {
        self.accepting.store(false, Ordering::Release);
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptureIngressStatus {
    Accepted,
    Paused,
    Dropped,
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
    fn observation_batch_round_trips_generation_sequence_gap_and_drop_evidence() {
        let batch = CaptureObservationBatch::new(
            CaptureProducerId::new("target-1").expect("producer id"),
            7,
            1,
            [
                CaptureObservationRecord::new(4, "windows.gdi.text-out", "Open")
                    .expect("first observation"),
                CaptureObservationRecord::new(6, "windows.console.write-console", "File")
                    .expect("second observation"),
            ],
        )
        .expect("observation batch");

        let encoded = batch.encode_json().expect("batch json");
        assert_eq!(
            CaptureObservationBatch::decode_json(&encoded).expect("decoded batch"),
            batch
        );
        assert_eq!(batch.producer_id().as_str(), "target-1");
        assert_eq!(batch.generation(), 7);
        assert_eq!(batch.dropped_total(), 1);
        assert_eq!(batch.records()[0].sequence(), 4);
        assert!(!encoded.contains("outputPath"));
        assert!(!encoded.contains("translation"));
    }

    #[test]
    fn observation_batch_rejects_unknown_unbounded_or_ambiguous_input() {
        let producer = || CaptureProducerId::new("worker-1").expect("producer id");
        let record = |sequence| {
            CaptureObservationRecord::new(sequence, "windows.uia.observe", "Name")
                .expect("observation")
        };

        assert_eq!(
            CaptureProducerId::new("target:1"),
            Err(CaptureError::InvalidObservationBatch)
        );
        assert_eq!(
            CaptureObservationRecord::new(
                1,
                "windows.uia.observe",
                "x".repeat(MAX_SOURCE_UNITS + 1),
            ),
            Err(CaptureError::InvalidObservationBatch)
        );
        assert_eq!(
            CaptureObservationBatch::new(producer(), 0, 0, [record(1)]),
            Err(CaptureError::InvalidObservationBatch)
        );
        assert_eq!(
            CaptureObservationBatch::new(producer(), 1, 0, [record(2), record(2)]),
            Err(CaptureError::InvalidObservationBatch)
        );
        assert_eq!(
            CaptureObservationBatch::new(
                producer(),
                1,
                0,
                (1..=MAX_OBSERVATION_BATCH_RECORDS + 1).map(|sequence| record(sequence as u64)),
            ),
            Err(CaptureError::InvalidObservationBatch)
        );

        let valid = CaptureObservationBatch::new(producer(), 1, 0, [record(1)])
            .expect("valid batch")
            .encode_json()
            .expect("valid json");
        let unknown = valid.replacen("\"records\"", "\"unknown\":true,\"records\"", 1);
        assert_eq!(
            CaptureObservationBatch::decode_json(&unknown),
            Err(CaptureError::InvalidObservationBatch)
        );
        assert_eq!(
            CaptureObservationBatch::decode_json(&"x".repeat(MAX_OBSERVATION_BATCH_BYTES + 1)),
            Err(CaptureError::InvalidObservationBatch)
        );
    }

    #[test]
    fn observation_cursor_rejects_replay_swaps_and_unexplained_sequence_gaps() {
        let configuration = CaptureProducerConfiguration::new(
            CaptureProducerId::new("target-1").expect("producer id"),
            7,
        )
        .expect("producer configuration");
        let mut cursor = CaptureObservationCursor::new(&configuration);
        let batch = |producer: &str, generation, dropped, sequence| {
            CaptureObservationBatch::new(
                CaptureProducerId::new(producer).expect("batch producer id"),
                generation,
                dropped,
                [
                    CaptureObservationRecord::new(sequence, "windows.gdi.text-out", "Open")
                        .expect("observation"),
                ],
            )
            .expect("observation batch")
        };

        assert_eq!(cursor.accept(&batch("target-1", 7, 0, 1)), Ok(0));
        assert_eq!(cursor.accept(&batch("target-1", 7, 1, 3)), Ok(1));
        assert_eq!(
            cursor.accept(&batch("target-1", 7, 1, 3)),
            Err(CaptureError::InvalidObservationBatch)
        );
        assert_eq!(
            cursor.accept(&batch("target-2", 7, 2, 4)),
            Err(CaptureError::InvalidObservationBatch)
        );
        assert_eq!(
            cursor.accept(&batch("target-1", 8, 2, 4)),
            Err(CaptureError::InvalidObservationBatch)
        );

        let mut unexplained = CaptureObservationCursor::new(&configuration);
        assert_eq!(
            unexplained.accept(&batch("target-1", 7, 0, 2)),
            Err(CaptureError::InvalidObservationBatch)
        );
    }

    #[test]
    fn batch_producer_drains_bounded_sorted_batches_without_blocking_ingress() {
        let (mut producer, ingress) = CaptureBatchProducer::start(
            CaptureProducerConfiguration::new(
                CaptureProducerId::new("target-7").expect("producer id"),
                3,
            )
            .expect("producer configuration"),
        )
        .expect("batch producer");
        let second_ingress = ingress.clone();
        let first = std::thread::spawn(move || ingress.try_observe("windows.gdi.text-out", "Open"));
        let second = std::thread::spawn(move || {
            second_ingress.try_observe("windows.console.write-console", "File")
        });
        assert_eq!(
            first.join().expect("first producer"),
            CaptureIngressStatus::Accepted
        );
        assert_eq!(
            second.join().expect("second producer"),
            CaptureIngressStatus::Accepted
        );

        let batch = producer.drain().expect("first batch");
        assert_eq!(batch.producer_id().as_str(), "target-7");
        assert_eq!(batch.generation(), 3);
        assert_eq!(batch.records().len(), 2);
        assert!(batch.records()[0].sequence() < batch.records()[1].sequence());
        assert!(producer.drain().expect("empty batch").records().is_empty());
    }

    #[test]
    fn batch_producer_preserves_pending_records_gaps_pause_and_owner_lifetime() {
        let (mut producer, ingress) = CaptureBatchProducer::start(
            CaptureProducerConfiguration::new(
                CaptureProducerId::new("worker-4").expect("producer id"),
                9,
            )
            .expect("producer configuration"),
        )
        .expect("batch producer");
        for index in 0..300 {
            assert_eq!(
                ingress.try_observe("windows.uia.observe", format!("Source {index}")),
                CaptureIngressStatus::Accepted
            );
        }
        assert_eq!(
            ingress.try_observe("windows.uia.observe", " "),
            CaptureIngressStatus::Dropped
        );
        assert_eq!(
            ingress.try_observe("windows.uia.observe", "After gap"),
            CaptureIngressStatus::Accepted
        );

        let first = producer.drain().expect("bounded first batch");
        let second = producer.drain().expect("pending second batch");
        assert_eq!(first.records().len(), MAX_OBSERVATION_BATCH_RECORDS);
        assert_eq!(second.records().len(), 45);
        assert!(
            first.records().last().expect("first tail").sequence()
                < second.records().first().expect("second head").sequence()
        );
        assert_eq!(second.dropped_total(), 1);
        assert!(second
            .records()
            .windows(2)
            .any(|records| records[1].sequence() > records[0].sequence() + 1));

        producer.set_paused(true);
        assert_eq!(
            ingress.try_observe("windows.uia.observe", "Paused"),
            CaptureIngressStatus::Paused
        );
        producer.set_paused(false);
        drop(producer);
        assert_eq!(
            ingress.try_observe("windows.uia.observe", "Too late"),
            CaptureIngressStatus::Dropped
        );
    }

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
    fn cloned_ingresses_merge_multiple_producers_through_one_checkpoint_owner() {
        let root = tempdir().expect("capture root");
        let output = root.path().join("capture.json");
        let sink = FileCaptureSink::start(
            CaptureConfiguration::new(
                CaptureSessionId::new("capture-ingress").expect("session id"),
                &output,
                10,
            )
            .expect("configuration"),
        )
        .expect("capture sink");
        let target_process = sink.ingress();
        let isolated_worker = sink.ingress();
        let stopped_worker = sink.ingress();

        let target_thread = std::thread::spawn(move || {
            target_process.try_observe("windows.gdi.text-out", "Target text")
        });
        let worker_thread = std::thread::spawn(move || {
            isolated_worker.try_observe("windows.uia.observe", "Worker text")
        });

        assert_eq!(
            target_thread.join().expect("target producer"),
            CaptureIngressStatus::Accepted
        );
        assert_eq!(
            worker_thread.join().expect("worker producer"),
            CaptureIngressStatus::Accepted
        );
        let catalog = sink.finish().expect("single capture owner");

        assert_eq!(catalog.entries().len(), 2);
        assert!(catalog.entries().iter().any(|entry| {
            entry.adapter_id() == "windows.gdi.text-out" && entry.source() == "Target text"
        }));
        assert!(catalog.entries().iter().any(|entry| {
            entry.adapter_id() == "windows.uia.observe" && entry.source() == "Worker text"
        }));
        assert_eq!(
            CaptureCatalog::read_current(&output).expect("single checkpoint"),
            catalog
        );
        assert_eq!(
            stopped_worker.try_observe("windows.uia.observe", "Too late"),
            CaptureIngressStatus::Dropped
        );
        assert_eq!(stopped_worker.dropped_observations(), 1);
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

        let ingress = sink.ingress();
        sink.set_paused(true);
        assert_eq!(
            ingress.try_observe("windows.gdi.text-out", "Ignored while paused"),
            CaptureIngressStatus::Paused
        );
        sink.set_paused(false);
        assert_eq!(
            ingress.try_observe("windows.gdi.text-out", "After resume"),
            CaptureIngressStatus::Accepted
        );
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

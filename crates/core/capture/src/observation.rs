use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const CAPTURE_OBSERVATION_BATCH_SCHEMA: &str = "glyphshift.capture-observation-batch/1";
pub(super) const MAX_ENTRIES: u32 = 250_000;
pub(super) const MAX_SOURCE_UNITS: usize = 16 * 1024;
const MAX_PRODUCER_ID_BYTES: usize = 256;
pub(super) const MAX_OBSERVATION_BATCH_RECORDS: usize = 256;
pub const MAX_OBSERVATION_BATCH_BYTES: usize = 4 * 1024 * 1024;

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
    pub(super) producer_id: CaptureProducerId,
    pub(super) generation: u64,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptureIngressStatus {
    Accepted,
    Paused,
    Dropped,
}

pub(super) fn safe_identifier(value: &str) -> bool {
    !value.is_empty()
        && value != "."
        && value != ".."
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_')
        })
}

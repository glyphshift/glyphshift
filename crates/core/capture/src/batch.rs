use crate::observation::{validate_observation_fields, MAX_OBSERVATION_BATCH_RECORDS};
use crate::{
    CaptureError, CaptureIngressStatus, CaptureObservationBatch, CaptureObservationRecord,
    CaptureProducerConfiguration, CaptureProducerId, CaptureTranslationContext,
};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TrySendError};
use std::sync::Arc;

// Runtime activation can expose a dense startup burst before the desktop-side supervisor has
// completed the activation handshake and started draining observations.
const QUEUE_CAPACITY: usize = 16_384;

/// Hot-path input owned by one [`CaptureBatchProducer`].
///
/// Accepted observations enter a bounded channel without waiting for the
/// transport consumer. Queue saturation and validation failures are reported
/// through the cumulative producer drop count.
#[derive(Clone)]
pub struct CaptureBatchIngress {
    sender: SyncSender<PendingCaptureObservation>,
    dropped: Arc<AtomicU64>,
    paused: Arc<AtomicBool>,
    accepting: Arc<AtomicBool>,
}

impl CaptureBatchIngress {
    #[must_use]
    pub fn try_observe(
        &self,
        adapter_id: impl Into<Box<str>>,
        source: impl Into<Box<str>>,
    ) -> CaptureIngressStatus {
        self.try_observe_with_context(adapter_id, source, None)
    }

    #[must_use]
    pub fn try_observe_with_context(
        &self,
        adapter_id: impl Into<Box<str>>,
        source: impl Into<Box<str>>,
        translation_context: Option<CaptureTranslationContext>,
    ) -> CaptureIngressStatus {
        if !self.accepting.load(Ordering::Acquire) {
            self.dropped.fetch_add(1, Ordering::Relaxed);
            return CaptureIngressStatus::Dropped;
        }
        if self.paused.load(Ordering::Relaxed) {
            return CaptureIngressStatus::Paused;
        }
        let Ok(observation) =
            PendingCaptureObservation::new(adapter_id, source, translation_context)
        else {
            self.dropped.fetch_add(1, Ordering::Relaxed);
            return CaptureIngressStatus::Dropped;
        };
        match self.sender.try_send(observation) {
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

struct PendingCaptureObservation {
    adapter_id: Box<str>,
    source: Box<str>,
    translation_context: Option<CaptureTranslationContext>,
}

impl PendingCaptureObservation {
    fn new(
        adapter_id: impl Into<Box<str>>,
        source: impl Into<Box<str>>,
        translation_context: Option<CaptureTranslationContext>,
    ) -> Result<Self, CaptureError> {
        let observation = Self {
            adapter_id: adapter_id.into(),
            source: source.into(),
            translation_context,
        };
        validate_observation_fields(&observation.adapter_id, &observation.source)?;
        if let Some(context) = &observation.translation_context {
            context.validate()?;
        }
        Ok(observation)
    }
}

/// Bounded observation producer drained by an out-of-process transport.
pub struct CaptureBatchProducer {
    producer_id: CaptureProducerId,
    generation: u64,
    receiver: Receiver<PendingCaptureObservation>,
    pending: VecDeque<CaptureObservationRecord>,
    last_sequence: u64,
    dropped: Arc<AtomicU64>,
    paused: Arc<AtomicBool>,
    accepting: Arc<AtomicBool>,
}

impl CaptureBatchProducer {
    pub fn start(
        configuration: CaptureProducerConfiguration,
    ) -> Result<(Self, CaptureBatchIngress), CaptureError> {
        let (sender, receiver) = sync_channel(QUEUE_CAPACITY);
        let dropped = Arc::new(AtomicU64::new(0));
        let paused = Arc::new(AtomicBool::new(false));
        let accepting = Arc::new(AtomicBool::new(true));
        let ingress = CaptureBatchIngress {
            sender,
            dropped: dropped.clone(),
            paused: paused.clone(),
            accepting: accepting.clone(),
        };
        Ok((
            Self {
                producer_id: configuration.producer_id,
                generation: configuration.generation,
                receiver,
                pending: VecDeque::new(),
                last_sequence: 0,
                dropped,
                paused,
                accepting,
            },
            ingress,
        ))
    }

    pub fn drain(&mut self) -> Result<CaptureObservationBatch, CaptureError> {
        for observation in self.receiver.try_iter() {
            let sequence = self
                .last_sequence
                .checked_add(1)
                .ok_or(CaptureError::WorkerUnavailable)?;
            let mut record = CaptureObservationRecord::new(
                sequence,
                observation.adapter_id,
                observation.source,
            )?;
            if let Some(context) = observation.translation_context {
                record = record.with_translation_context(context)?;
            }
            self.last_sequence = sequence;
            self.pending.push_back(record);
        }
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

use crate::observation::MAX_OBSERVATION_BATCH_RECORDS;
use crate::{
    CaptureError, CaptureIngressStatus, CaptureObservationBatch, CaptureObservationRecord,
    CaptureProducerConfiguration, CaptureProducerId,
};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TrySendError};
use std::sync::{Arc, RwLock};

const QUEUE_CAPACITY: usize = 8_192;

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

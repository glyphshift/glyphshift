use glyphshift_capture::{
    CaptureBatchIngress, CaptureBatchProducer, CaptureProducerConfiguration, CaptureProducerId,
};
use glyphshift_isolated_worker_sdk::{
    serve_stdio, IsolatedWorker, WireWorkerHealthReport, WorkerActivation, WorkerError,
};

#[derive(Default)]
struct SyntheticIsolatedWorker {
    adapter_id: Option<Box<str>>,
    generation: u64,
    publication_generation: u64,
    producer: Option<CaptureBatchProducer>,
    ingress: Option<CaptureBatchIngress>,
    uia: UiaObserver,
    hang_on_query: bool,
    deactivated: bool,
}

impl SyntheticIsolatedWorker {
    fn require_active(&self) -> Result<(), WorkerError> {
        if self.producer.is_some() && !self.deactivated {
            Ok(())
        } else {
            Err(WorkerError::new("worker_not_active"))
        }
    }

    fn observe(&self, source: &str) -> Result<(), WorkerError> {
        let adapter_id = self
            .adapter_id
            .as_deref()
            .ok_or_else(|| WorkerError::new("worker_not_active"))?;
        let ingress = self
            .ingress
            .as_ref()
            .ok_or_else(|| WorkerError::new("worker_not_active"))?;
        let _ = ingress.try_observe(adapter_id, source);
        Ok(())
    }

    fn observe_snapshot(&mut self, snapshot: UiaElementSnapshot) -> Result<(), WorkerError> {
        let UiaObservationOutcome::Observed(observation) = self.uia.observe(snapshot) else {
            return Ok(());
        };
        self.observe(observation.text())
    }
}

impl IsolatedWorker for SyntheticIsolatedWorker {
    fn activate(&mut self, activation: &WorkerActivation) -> Result<(), WorkerError> {
        if activation.target_grant.payload == "target:permission-denied" {
            return Err(WorkerError::new("uia_permission_denied"));
        }
        if self.producer.is_some()
            || activation.adapter_id != "windows.uia.synthetic"
            || activation.target_grant.platform != "synthetic-process-v1"
            || !matches!(
                activation.target_grant.payload.as_str(),
                "target:authorized" | "target:hang-on-query" | "target:hang-once"
            )
        {
            return Err(WorkerError::new("activation_rejected"));
        }
        let producer_id = CaptureProducerId::new(activation.producer_id.clone())
            .map_err(|_| WorkerError::new("invalid_producer"))?;
        let configuration =
            CaptureProducerConfiguration::new(producer_id, activation.producer_generation)
                .map_err(|_| WorkerError::new("invalid_generation"))?;
        let (producer, ingress) = CaptureBatchProducer::start(configuration)
            .map_err(|_| WorkerError::new("producer_unavailable"))?;
        self.adapter_id = Some(activation.adapter_id.clone().into());
        self.generation = activation.producer_generation;
        self.publication_generation = activation.publication_generation;
        self.hang_on_query = activation.target_grant.payload == "target:hang-on-query"
            || (activation.target_grant.payload == "target:hang-once"
                && activation.producer_generation == 1);
        self.producer = Some(producer);
        self.ingress = Some(ingress);
        self.observe_snapshot(UiaElementSnapshot::new("window-title").with_name("Window title"))?;
        self.observe_snapshot(UiaElementSnapshot::new("document").with_text("Document text"))?;
        self.observe_snapshot(UiaElementSnapshot::new("field").with_value("Field value"))?;
        Ok(())
    }

    fn control_capture(&mut self, paused: bool) -> Result<(), WorkerError> {
        self.require_active()?;
        self.producer
            .as_ref()
            .ok_or_else(|| WorkerError::new("worker_not_active"))?
            .set_paused(paused);
        if !paused {
            self.observe_snapshot(
                UiaElementSnapshot::new("resumed-label").with_name("Resumed label"),
            )?;
        }
        Ok(())
    }

    fn update_generation(&mut self, publication_generation: u64) -> Result<u64, WorkerError> {
        self.require_active()?;
        if publication_generation <= self.publication_generation {
            return Err(WorkerError::new("generation_not_monotonic"));
        }
        self.publication_generation = publication_generation;
        Ok(publication_generation)
    }

    fn query_observations(&mut self) -> Result<String, WorkerError> {
        if self.hang_on_query {
            std::thread::sleep(std::time::Duration::from_secs(10));
        }
        self.producer
            .as_mut()
            .ok_or_else(|| WorkerError::new("worker_not_active"))?
            .drain()
            .and_then(|batch| batch.encode_json())
            .map_err(|_| WorkerError::new("observation_unavailable"))
    }

    fn health(&mut self) -> Result<WireWorkerHealthReport, WorkerError> {
        self.require_active()?;
        Ok(WireWorkerHealthReport::healthy())
    }

    fn deactivate(&mut self) -> Result<u64, WorkerError> {
        self.require_active()?;
        self.producer
            .as_ref()
            .ok_or_else(|| WorkerError::new("worker_not_active"))?
            .set_paused(false);
        self.observe_snapshot(
            UiaElementSnapshot::new("handler-removal-tail").with_name("Handler removal tail"),
        )?;
        self.producer
            .as_ref()
            .ok_or_else(|| WorkerError::new("worker_not_active"))?
            .set_paused(true);
        self.deactivated = true;
        Ok(self.generation)
    }
}

fn main() -> std::io::Result<()> {
    serve_stdio(SyntheticIsolatedWorker::default())
}
use glyphshift_adapter_uia::{UiaElementSnapshot, UiaObservationOutcome, UiaObserver};

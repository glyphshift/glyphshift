//! Supervised process transport for observe-only isolated workers.

use glyphshift_adapter_registry::{AdapterBinding, AdapterHostBinding, PackageArtifactId};
use glyphshift_capture::{
    CaptureIngress, CaptureObservationBatch, CaptureObservationCursor, CaptureProducerConfiguration,
};
use glyphshift_domain::AdapterId;
use glyphshift_isolated_worker_sdk::{
    Request, RequestEnvelope, Response, ResponseEnvelope, WireWorkerHealth, WireWorkerHealthReport,
    WorkerActivation, WorkerTargetGrant, PROTOCOL_SCHEMA,
};
use glyphshift_protocol::ControllerTransport;
use glyphshift_runtime_contract::RuntimePublication;
use glyphshift_session::{
    AdapterDeactivation, AdapterHostPort, BoundAdapter, BoundFeature, HostActivation,
    HostDeactivation, HostFailure, HostGenerationReport, HostHealthReport, HostOperationFailure,
    SessionDiagnostic, SessionId, TargetInstance, TargetInstanceId,
};
use glyphshift_target_process_host::TargetProcessHost;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, sync_channel, Receiver, RecvTimeoutError, SyncSender};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const MAX_DRAIN_BATCHES: usize = 64;
const MAX_HEALTH_CODE_BYTES: usize = 128;
const MAX_TARGET_GRANT_BYTES: usize = 4_096;
const WORKER_POLL_INTERVAL: Duration = Duration::from_millis(250);
const WORKER_RESTART_WINDOW: Duration = Duration::from_secs(60);
const MAX_WORKER_RESTARTS: usize = 3;
const SUPERVISOR_RUNNING: u64 = 0;
const SUPERVISOR_RESTART_EXHAUSTED: u64 = 1;
const SUPERVISOR_RESTART_FAILED: u64 = 2;

#[derive(Clone, Debug)]
pub struct WorkerArtifact {
    executable: PathBuf,
}

#[derive(Clone, Debug, Default)]
pub struct WorkerArtifactCatalog {
    workers: BTreeMap<PackageArtifactId, WorkerArtifact>,
}

impl WorkerArtifactCatalog {
    pub fn new(
        workers: impl IntoIterator<Item = (PackageArtifactId, PathBuf)>,
    ) -> Result<Self, WorkerHostError> {
        let mut indexed = BTreeMap::new();
        for (artifact_id, executable) in workers {
            let artifact = WorkerArtifact::open(executable)?;
            if indexed.insert(artifact_id, artifact).is_some() {
                return Err(WorkerHostError::ArtifactUnavailable);
            }
        }
        Ok(Self { workers: indexed })
    }
}

impl WorkerArtifact {
    pub fn open(executable: impl Into<PathBuf>) -> Result<Self, WorkerHostError> {
        let executable = executable.into();
        if !executable.is_absolute() || !executable.is_file() {
            return Err(WorkerHostError::ArtifactUnavailable);
        }
        Ok(Self { executable })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkerHostError {
    ArtifactUnavailable,
    SpawnFailed,
    Timeout,
    Crashed,
    MalformedMessage,
    HandshakeRejected,
    WorkerRejected(Box<str>),
    InvalidObservation,
    DrainLimitExceeded,
    DeactivationRejected,
    RestartLimitExceeded,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkerHealth {
    Healthy,
    Degraded,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkerHealthReport {
    state: WorkerHealth,
    code: Option<Box<str>>,
}

impl WorkerHealthReport {
    #[must_use]
    pub const fn state(&self) -> WorkerHealth {
        self.state
    }

    #[must_use]
    pub fn code(&self) -> Option<&str> {
        self.code.as_deref()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WorkerDrainReport {
    records: u64,
    dropped: u64,
}

impl WorkerDrainReport {
    #[must_use]
    pub const fn records(self) -> u64 {
        self.records
    }

    #[must_use]
    pub const fn dropped(self) -> u64 {
        self.dropped
    }

    fn merge(&mut self, other: Self) {
        self.records = self.records.saturating_add(other.records);
        self.dropped = self.dropped.saturating_add(other.dropped);
    }
}

pub struct ProcessIsolatedWorker {
    process: WorkerProcess,
    producer: CaptureProducerConfiguration,
    cursor: CaptureObservationCursor,
    deactivated: bool,
}

impl ProcessIsolatedWorker {
    pub fn spawn(
        artifact: WorkerArtifact,
        timeout: Duration,
        adapter_id: &AdapterId,
        target_grant: WorkerTargetGrant,
        producer: CaptureProducerConfiguration,
        publication_generation: u64,
    ) -> Result<Self, WorkerHostError> {
        if target_grant.platform.trim().is_empty()
            || target_grant.payload.trim().is_empty()
            || target_grant.platform.len() > 128
            || target_grant.payload.len() > MAX_TARGET_GRANT_BYTES
            || publication_generation == 0
        {
            return Err(WorkerHostError::HandshakeRejected);
        }
        let mut process = WorkerProcess::spawn(&artifact.executable, timeout)?;
        match process.round_trip(Request::Handshake {
            activation: WorkerActivation {
                adapter_id: adapter_id.as_str().into(),
                target_grant,
                producer_id: producer.producer_id().as_str().into(),
                producer_generation: producer.generation(),
                publication_generation,
            },
        })? {
            Response::Hello {
                adapter_id: acknowledged_adapter,
                producer_id,
                producer_generation,
                publication_generation: acknowledged_publication,
            } if acknowledged_adapter == adapter_id.as_str()
                && producer_id == producer.producer_id().as_str()
                && producer_generation == producer.generation()
                && acknowledged_publication == publication_generation => {}
            _ => return Err(WorkerHostError::HandshakeRejected),
        }
        Ok(Self {
            cursor: CaptureObservationCursor::new(&producer),
            producer,
            process,
            deactivated: false,
        })
    }

    pub fn set_paused(&mut self, paused: bool) -> Result<(), WorkerHostError> {
        match self
            .process
            .round_trip(Request::ControlCapture { paused })?
        {
            Response::CaptureControlled {
                paused: acknowledged,
            } if acknowledged == paused => Ok(()),
            _ => Err(WorkerHostError::MalformedMessage),
        }
    }

    pub fn update_generation(
        &mut self,
        publication_generation: u64,
    ) -> Result<u64, WorkerHostError> {
        if publication_generation == 0 {
            return Err(WorkerHostError::MalformedMessage);
        }
        match self.process.round_trip(Request::UpdateGeneration {
            publication_generation,
        })? {
            Response::GenerationApplied {
                publication_generation: acknowledged,
            } if acknowledged == publication_generation => Ok(acknowledged),
            _ => Err(WorkerHostError::MalformedMessage),
        }
    }

    pub fn health(&mut self) -> Result<WorkerHealthReport, WorkerHostError> {
        let Response::Health { report } = self.process.round_trip(Request::QueryHealth)? else {
            return Err(WorkerHostError::MalformedMessage);
        };
        decode_health(report)
    }

    pub fn drain_once(
        &mut self,
        ingress: &CaptureIngress,
    ) -> Result<WorkerDrainReport, WorkerHostError> {
        let Response::Observations { batch_json } =
            self.process.round_trip(Request::QueryObservations)?
        else {
            return Err(WorkerHostError::MalformedMessage);
        };
        let batch = CaptureObservationBatch::decode_json(&batch_json)
            .map_err(|_| WorkerHostError::InvalidObservation)?;
        let dropped = self
            .cursor
            .accept(&batch)
            .map_err(|_| WorkerHostError::InvalidObservation)?;
        ingress.report_dropped(dropped);
        for record in batch.records() {
            let _ = ingress.try_observe(record.adapter_id(), record.source());
        }
        Ok(WorkerDrainReport {
            records: batch.records().len() as u64,
            dropped,
        })
    }

    pub fn drain_until_empty(
        &mut self,
        ingress: &CaptureIngress,
    ) -> Result<WorkerDrainReport, WorkerHostError> {
        let mut report = WorkerDrainReport::default();
        for _ in 0..MAX_DRAIN_BATCHES {
            let batch = self.drain_once(ingress)?;
            report.merge(batch);
            if batch.records == 0 {
                return Ok(report);
            }
        }
        Err(WorkerHostError::DrainLimitExceeded)
    }

    /// Freezes callbacks, drains queued observations, obtains the handler-removal acknowledgement,
    /// and drains the tail produced before removal completed.
    pub fn deactivate(
        &mut self,
        ingress: &CaptureIngress,
    ) -> Result<WorkerDrainReport, WorkerHostError> {
        if self.deactivated {
            return Err(WorkerHostError::DeactivationRejected);
        }
        self.set_paused(true)?;
        let mut report = self.drain_until_empty(ingress)?;
        match self.process.round_trip(Request::Deactivate)? {
            Response::Deactivated {
                producer_generation,
            } if producer_generation == self.producer.generation() => {}
            _ => return Err(WorkerHostError::DeactivationRejected),
        }
        report.merge(self.drain_until_empty(ingress)?);
        self.deactivated = true;
        self.process.terminate();
        Ok(report)
    }
}

impl Drop for ProcessIsolatedWorker {
    fn drop(&mut self) {
        self.process.terminate();
    }
}

enum WorkerSupervisorCommand {
    SetPaused {
        paused: bool,
        reply: SyncSender<Result<(), WorkerHostError>>,
    },
    UpdateGeneration {
        generation: u64,
        reply: SyncSender<Result<u64, WorkerHostError>>,
    },
    Health {
        reply: SyncSender<Result<WorkerHealthReport, WorkerHostError>>,
    },
    Finish {
        reply: SyncSender<Result<WorkerDrainReport, WorkerHostError>>,
    },
}

struct WorkerSupervisor {
    commands: SyncSender<WorkerSupervisorCommand>,
    worker: Option<JoinHandle<()>>,
    terminal: Arc<AtomicU64>,
}

struct WorkerRestartContext {
    artifact: WorkerArtifact,
    timeout: Duration,
    adapter_id: AdapterId,
    target_grant: WorkerTargetGrant,
    generation_allocator: Arc<AtomicU64>,
}

impl WorkerSupervisor {
    #[allow(clippy::too_many_arguments)]
    fn start(
        artifact: WorkerArtifact,
        timeout: Duration,
        adapter_id: &AdapterId,
        target_grant: WorkerTargetGrant,
        producer: CaptureProducerConfiguration,
        publication_generation: u64,
        ingress: CaptureIngress,
        generation_allocator: Arc<AtomicU64>,
    ) -> Result<Self, WorkerHostError> {
        let worker = ProcessIsolatedWorker::spawn(
            artifact.clone(),
            timeout,
            adapter_id,
            target_grant.clone(),
            producer,
            publication_generation,
        )?;
        let adapter_id = adapter_id.clone();
        let restart = WorkerRestartContext {
            artifact,
            timeout,
            adapter_id,
            target_grant,
            generation_allocator,
        };
        let (commands, receiver) = sync_channel(8);
        let terminal = Arc::new(AtomicU64::new(SUPERVISOR_RUNNING));
        let worker_terminal = Arc::clone(&terminal);
        let worker = thread::Builder::new()
            .name("glyphshift-isolated-worker-supervisor".into())
            .spawn(move || {
                worker_supervisor_loop(
                    worker,
                    ingress,
                    receiver,
                    restart,
                    publication_generation,
                    worker_terminal,
                )
            })
            .map_err(|_| WorkerHostError::SpawnFailed)?;
        Ok(Self {
            commands,
            worker: Some(worker),
            terminal,
        })
    }

    fn set_paused(&self, paused: bool) -> Result<(), WorkerHostError> {
        let (reply, response) = sync_channel(1);
        self.commands
            .send(WorkerSupervisorCommand::SetPaused { paused, reply })
            .map_err(|_| WorkerHostError::Crashed)?;
        response.recv().map_err(|_| WorkerHostError::Crashed)?
    }

    fn update_generation(&self, generation: u64) -> Result<u64, WorkerHostError> {
        let (reply, response) = sync_channel(1);
        self.commands
            .send(WorkerSupervisorCommand::UpdateGeneration { generation, reply })
            .map_err(|_| WorkerHostError::Crashed)?;
        response.recv().map_err(|_| WorkerHostError::Crashed)?
    }

    fn health(&self) -> Result<WorkerHealthReport, WorkerHostError> {
        let (reply, response) = sync_channel(1);
        if self
            .commands
            .send(WorkerSupervisorCommand::Health { reply })
            .is_err()
        {
            return self.terminal_health();
        }
        response.recv().unwrap_or_else(|_| self.terminal_health())
    }

    fn terminal_health(&self) -> Result<WorkerHealthReport, WorkerHostError> {
        let code = match self.terminal.load(Ordering::Acquire) {
            SUPERVISOR_RESTART_EXHAUSTED => "isolated_worker_restart_exhausted",
            SUPERVISOR_RESTART_FAILED => "isolated_worker_restart_failed",
            _ => return Err(WorkerHostError::Crashed),
        };
        Ok(WorkerHealthReport {
            state: WorkerHealth::Degraded,
            code: Some(code.into()),
        })
    }

    fn finish(&mut self) -> Result<WorkerDrainReport, WorkerHostError> {
        let (reply, response) = sync_channel(1);
        let result = if self
            .commands
            .send(WorkerSupervisorCommand::Finish { reply })
            .is_ok()
        {
            response.recv().map_err(|_| WorkerHostError::Crashed)?
        } else {
            Err(WorkerHostError::Crashed)
        };
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        result
    }
}

impl Drop for WorkerSupervisor {
    fn drop(&mut self) {
        let _ = self.finish();
    }
}

fn worker_supervisor_loop(
    mut worker: ProcessIsolatedWorker,
    ingress: CaptureIngress,
    commands: Receiver<WorkerSupervisorCommand>,
    restart: WorkerRestartContext,
    mut publication_generation: u64,
    terminal: Arc<AtomicU64>,
) {
    let mut paused = false;
    let mut restart_times = VecDeque::new();
    loop {
        match commands.recv_timeout(WORKER_POLL_INTERVAL) {
            Ok(WorkerSupervisorCommand::SetPaused {
                paused: requested,
                reply,
            }) => {
                let result = worker.set_paused(requested).and_then(|()| {
                    if requested {
                        worker.drain_until_empty(&ingress).map(|_| ())
                    } else {
                        Ok(())
                    }
                });
                if result.is_ok() {
                    paused = requested;
                }
                let _ = reply.send(result);
            }
            Ok(WorkerSupervisorCommand::UpdateGeneration { generation, reply }) => {
                let result = worker.update_generation(generation);
                if result.is_ok() {
                    publication_generation = generation;
                }
                let _ = reply.send(result);
            }
            Ok(WorkerSupervisorCommand::Health { reply }) => {
                let _ = reply.send(worker.health());
            }
            Ok(WorkerSupervisorCommand::Finish { reply }) => {
                let _ = reply.send(worker.deactivate(&ingress));
                return;
            }
            Err(RecvTimeoutError::Timeout) => {
                if worker.drain_once(&ingress).is_err() {
                    let restarted = match restart_worker(
                        &restart,
                        publication_generation,
                        paused,
                        &mut restart_times,
                    ) {
                        Ok(restarted) => restarted,
                        Err(WorkerHostError::RestartLimitExceeded) => {
                            terminal.store(SUPERVISOR_RESTART_EXHAUSTED, Ordering::Release);
                            return;
                        }
                        Err(_) => {
                            terminal.store(SUPERVISOR_RESTART_FAILED, Ordering::Release);
                            return;
                        }
                    };
                    worker = restarted;
                }
            }
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}

fn restart_worker(
    restart: &WorkerRestartContext,
    publication_generation: u64,
    paused: bool,
    restart_times: &mut VecDeque<Instant>,
) -> Result<ProcessIsolatedWorker, WorkerHostError> {
    let now = Instant::now();
    while restart_times
        .front()
        .is_some_and(|started| now.duration_since(*started) >= WORKER_RESTART_WINDOW)
    {
        restart_times.pop_front();
    }
    if restart_times.len() >= MAX_WORKER_RESTARTS {
        return Err(WorkerHostError::RestartLimitExceeded);
    }
    let generation = allocate_producer_generation(&restart.generation_allocator)?;
    let producer = CaptureProducerConfiguration::new(
        glyphshift_capture::CaptureProducerId::new(format!("isolated-{generation}"))
            .map_err(|_| WorkerHostError::SpawnFailed)?,
        generation,
    )
    .map_err(|_| WorkerHostError::SpawnFailed)?;
    let mut worker = ProcessIsolatedWorker::spawn(
        restart.artifact.clone(),
        restart.timeout,
        &restart.adapter_id,
        restart.target_grant.clone(),
        producer,
        publication_generation,
    )?;
    if paused {
        worker.set_paused(true)?;
    }
    restart_times.push_back(now);
    Ok(worker)
}

fn allocate_producer_generation(allocator: &AtomicU64) -> Result<u64, WorkerHostError> {
    allocator
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |generation| {
            generation.checked_add(1)
        })
        .map_err(|_| WorkerHostError::SpawnFailed)
}

/// Adapter Host for one isolated worker process per target and Adapter binding.
pub struct IsolatedWorkerHost {
    artifacts: WorkerArtifactCatalog,
    targets: BTreeMap<TargetInstanceId, WorkerTargetGrant>,
    workers: BTreeMap<TargetInstanceId, BTreeMap<BoundAdapter, WorkerSupervisor>>,
    capture_ingress: CaptureIngress,
    timeout: Duration,
    next_producer_generation: Arc<AtomicU64>,
}

impl IsolatedWorkerHost {
    #[must_use]
    pub fn new(
        artifacts: WorkerArtifactCatalog,
        capture_ingress: CaptureIngress,
        timeout: Duration,
    ) -> Self {
        Self {
            artifacts,
            targets: BTreeMap::new(),
            workers: BTreeMap::new(),
            capture_ingress,
            timeout,
            next_producer_generation: Arc::new(AtomicU64::new(1)),
        }
    }

    pub fn register_target(
        &mut self,
        target_instance_id: TargetInstanceId,
        target_grant: WorkerTargetGrant,
    ) {
        self.targets.insert(target_instance_id, target_grant);
    }

    fn isolated_bindings(bindings: &[AdapterBinding]) -> impl Iterator<Item = &AdapterBinding> {
        bindings
            .iter()
            .filter(|binding| matches!(binding.host, AdapterHostBinding::IsolatedWorker { .. }))
    }

    fn isolated_features(bindings: &[AdapterBinding]) -> Vec<BoundFeature> {
        Self::isolated_bindings(bindings)
            .flat_map(|binding| {
                binding.features.iter().copied().map(|feature| {
                    BoundFeature::new(binding.adapter_id.clone(), binding.version, feature)
                })
            })
            .collect()
    }

    fn isolated_adapters(bindings: &[AdapterBinding]) -> Vec<BoundAdapter> {
        Self::isolated_bindings(bindings)
            .map(|binding| BoundAdapter::new(binding.adapter_id.clone(), binding.version))
            .collect()
    }

    fn activate_generation(
        &mut self,
        target: &TargetInstance,
        bindings: &[AdapterBinding],
        publication_generation: u64,
    ) -> Result<HostActivation, HostFailure> {
        let target_grant = self
            .targets
            .get(target.id())
            .cloned()
            .ok_or(HostFailure::Unavailable)?;
        if self.workers.contains_key(target.id()) {
            return Err(HostFailure::Unavailable);
        }
        let mut started = BTreeMap::new();
        for binding in Self::isolated_bindings(bindings) {
            let AdapterHostBinding::IsolatedWorker { executable } = &binding.host else {
                continue;
            };
            let artifact = self
                .artifacts
                .workers
                .get(executable)
                .cloned()
                .ok_or(HostFailure::Unavailable)?;
            let producer_generation = allocate_producer_generation(&self.next_producer_generation)
                .map_err(|_| HostFailure::Unavailable)?;
            let producer = CaptureProducerConfiguration::new(
                glyphshift_capture::CaptureProducerId::new(format!(
                    "isolated-{producer_generation}"
                ))
                .map_err(|_| HostFailure::Unavailable)?,
                producer_generation,
            )
            .map_err(|_| HostFailure::Unavailable)?;
            let adapter = BoundAdapter::new(binding.adapter_id.clone(), binding.version);
            let supervisor = WorkerSupervisor::start(
                artifact,
                self.timeout,
                &binding.adapter_id,
                target_grant.clone(),
                producer,
                publication_generation,
                self.capture_ingress.clone(),
                Arc::clone(&self.next_producer_generation),
            )
            .map_err(map_worker_error)?;
            started.insert(adapter, supervisor);
        }
        self.workers.insert(target.id().clone(), started);
        Ok(HostActivation::connected(Self::isolated_features(bindings)))
    }
}

fn map_worker_error(error: WorkerHostError) -> HostFailure {
    match error {
        WorkerHostError::HandshakeRejected | WorkerHostError::MalformedMessage => {
            HostFailure::HandshakeRejected
        }
        WorkerHostError::WorkerRejected(code) if code.as_ref() == "uia_permission_denied" => {
            HostFailure::OperationRejected(HostOperationFailure::IsolatedWorkerPermissionDenied)
        }
        WorkerHostError::Timeout => {
            HostFailure::OperationRejected(HostOperationFailure::IsolatedWorkerTimeout)
        }
        _ => HostFailure::Unavailable,
    }
}

impl AdapterHostPort for IsolatedWorkerHost {
    fn activate(
        &mut self,
        target: &TargetInstance,
        bindings: &[AdapterBinding],
    ) -> Result<HostActivation, HostFailure> {
        self.activate_generation(target, bindings, 1)
    }

    fn activate_runtime(
        &mut self,
        target: &TargetInstance,
        bindings: &[AdapterBinding],
        publication: &RuntimePublication,
    ) -> Result<HostActivation, HostFailure> {
        self.activate_generation(target, bindings, publication.generation().value())
    }

    fn update(
        &mut self,
        _session_id: SessionId,
        target: &TargetInstance,
        bindings: &[AdapterBinding],
        generation: glyphshift_domain::Generation,
    ) -> Result<HostGenerationReport, HostFailure> {
        let workers = self
            .workers
            .get(target.id())
            .ok_or(HostFailure::Unavailable)?;
        let mut acknowledgements = Vec::new();
        for binding in Self::isolated_bindings(bindings) {
            let adapter = BoundAdapter::new(binding.adapter_id.clone(), binding.version);
            let acknowledged = workers
                .get(&adapter)
                .ok_or(HostFailure::Unavailable)?
                .update_generation(generation.value())
                .map_err(map_worker_error)?;
            let acknowledged = glyphshift_domain::Generation::new(acknowledged);
            acknowledgements.extend(binding.features.iter().copied().map(|feature| {
                (
                    BoundFeature::new(binding.adapter_id.clone(), binding.version, feature),
                    acknowledged,
                )
            }));
        }
        Ok(HostGenerationReport::isolated(acknowledgements))
    }

    fn control_capture(
        &mut self,
        _session_id: SessionId,
        target: &TargetInstance,
        paused: bool,
    ) -> Result<(), HostFailure> {
        for supervisor in self
            .workers
            .get(target.id())
            .ok_or(HostFailure::Unavailable)?
            .values()
        {
            supervisor.set_paused(paused).map_err(map_worker_error)?;
        }
        Ok(())
    }

    fn health(
        &mut self,
        _session_id: SessionId,
        target: &TargetInstance,
        bindings: &[AdapterBinding],
    ) -> Result<HostHealthReport, HostFailure> {
        let workers = self
            .workers
            .get(target.id())
            .ok_or(HostFailure::Unavailable)?;
        let mut failed = BTreeSet::new();
        let mut diagnostics = Vec::new();
        for adapter in Self::isolated_adapters(bindings) {
            match workers
                .get(&adapter)
                .and_then(|worker| worker.health().ok())
            {
                Some(report) if report.state() == WorkerHealth::Healthy => {}
                Some(report) => {
                    failed.insert(adapter);
                    diagnostics.push(SessionDiagnostic::new(
                        report.code().unwrap_or("isolated_worker_degraded"),
                    ));
                }
                None => {
                    failed.insert(adapter);
                    diagnostics.push(SessionDiagnostic::new("isolated_worker_unavailable"));
                }
            }
        }
        Ok(HostHealthReport::reported(failed, diagnostics))
    }

    fn deactivate(
        &mut self,
        _session_id: SessionId,
        target: &TargetInstance,
        bindings: &[AdapterBinding],
        _plan: &[AdapterDeactivation],
    ) -> Result<HostDeactivation, HostFailure> {
        let mut completed = BTreeSet::new();
        let mut failed = BTreeSet::new();
        if let Some(workers) = self.workers.remove(target.id()) {
            for (adapter, mut worker) in workers {
                if worker.finish().is_ok() {
                    completed.insert(adapter);
                } else {
                    failed.insert(adapter);
                }
            }
        } else {
            failed.extend(Self::isolated_adapters(bindings));
        }
        Ok(HostDeactivation::reported(completed, failed))
    }

    fn release(
        &mut self,
        _session_id: SessionId,
        target: &TargetInstance,
        _bindings: &[AdapterBinding],
    ) -> Result<(), HostFailure> {
        if let Some(workers) = self.workers.remove(target.id()) {
            for (_, mut worker) in workers {
                let _ = worker.finish();
            }
        }
        self.targets.remove(target.id());
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct ActivePlacements {
    target_process: bool,
    isolated_worker: bool,
}

/// One Session-facing host that routes each binding to its declared placement.
pub struct HybridAdapterHost<T> {
    target_process: TargetProcessHost<T>,
    isolated_worker: Option<IsolatedWorkerHost>,
    active: BTreeMap<TargetInstanceId, ActivePlacements>,
}

impl<T> HybridAdapterHost<T> {
    #[must_use]
    pub fn new(target_process: TargetProcessHost<T>) -> Self {
        Self {
            target_process,
            isolated_worker: None,
            active: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn with_isolated_worker(mut self, isolated_worker: IsolatedWorkerHost) -> Self {
        self.isolated_worker = Some(isolated_worker);
        self
    }

    fn placements(bindings: &[AdapterBinding]) -> ActivePlacements {
        ActivePlacements {
            target_process: bindings
                .iter()
                .any(|binding| matches!(binding.host, AdapterHostBinding::TargetProcess { .. })),
            isolated_worker: bindings
                .iter()
                .any(|binding| matches!(binding.host, AdapterHostBinding::IsolatedWorker { .. })),
        }
    }

    fn features(bindings: &[AdapterBinding]) -> Vec<BoundFeature> {
        bindings
            .iter()
            .flat_map(|binding| {
                binding.features.iter().copied().map(|feature| {
                    BoundFeature::new(binding.adapter_id.clone(), binding.version, feature)
                })
            })
            .collect()
    }

    fn adapters_for(
        bindings: &[AdapterBinding],
        placement: fn(&AdapterHostBinding) -> bool,
    ) -> Vec<BoundAdapter> {
        bindings
            .iter()
            .filter(|binding| placement(&binding.host))
            .map(|binding| BoundAdapter::new(binding.adapter_id.clone(), binding.version))
            .collect()
    }
}

impl<T: ControllerTransport + Send + 'static> AdapterHostPort for HybridAdapterHost<T> {
    fn activate(
        &mut self,
        target: &TargetInstance,
        bindings: &[AdapterBinding],
    ) -> Result<HostActivation, HostFailure> {
        let placements = Self::placements(bindings);
        if placements.target_process || self.active.contains_key(target.id()) {
            return Err(HostFailure::HandshakeRejected);
        }
        if placements.isolated_worker {
            self.isolated_worker
                .as_mut()
                .ok_or(HostFailure::Unavailable)?
                .activate(target, bindings)?;
        }
        self.active.insert(target.id().clone(), placements);
        Ok(HostActivation::connected(Self::features(bindings)))
    }

    fn activate_runtime(
        &mut self,
        target: &TargetInstance,
        bindings: &[AdapterBinding],
        publication: &RuntimePublication,
    ) -> Result<HostActivation, HostFailure> {
        let placements = Self::placements(bindings);
        if self.active.contains_key(target.id()) {
            return Err(HostFailure::Unavailable);
        }
        if placements.target_process {
            self.target_process
                .activate_runtime(target, bindings, publication)?;
        }
        if placements.isolated_worker {
            let isolated = self
                .isolated_worker
                .as_mut()
                .ok_or(HostFailure::Unavailable);
            if isolated
                .and_then(|host| host.activate_runtime(target, bindings, publication))
                .is_err()
            {
                if placements.target_process {
                    let _ =
                        self.target_process
                            .deactivate(SessionId::new(0), target, bindings, &[]);
                }
                return Err(HostFailure::Unavailable);
            }
        }
        self.active.insert(target.id().clone(), placements);
        Ok(HostActivation::connected(Self::features(bindings)))
    }

    fn update(
        &mut self,
        session_id: SessionId,
        target: &TargetInstance,
        bindings: &[AdapterBinding],
        generation: glyphshift_domain::Generation,
    ) -> Result<HostGenerationReport, HostFailure> {
        let placements = self
            .active
            .get(target.id())
            .copied()
            .ok_or(HostFailure::Unavailable)?;
        if placements.target_process {
            return Err(HostFailure::Unavailable);
        }
        if placements.isolated_worker {
            self.isolated_worker
                .as_mut()
                .ok_or(HostFailure::Unavailable)?
                .update(session_id, target, bindings, generation)?;
        }
        Ok(HostGenerationReport::reported(
            None,
            Self::features(bindings)
                .into_iter()
                .map(|feature| (feature, generation)),
        ))
    }

    fn update_runtime(
        &mut self,
        session_id: SessionId,
        target: &TargetInstance,
        bindings: &[AdapterBinding],
        publication: &RuntimePublication,
    ) -> Result<HostGenerationReport, HostFailure> {
        let placements = self
            .active
            .get(target.id())
            .copied()
            .ok_or(HostFailure::Unavailable)?;
        if placements.target_process {
            self.target_process
                .update_runtime(session_id, target, bindings, publication)?;
        }
        if placements.isolated_worker {
            self.isolated_worker
                .as_mut()
                .ok_or(HostFailure::Unavailable)?
                .update_runtime(session_id, target, bindings, publication)?;
        }
        let isolated = bindings
            .iter()
            .filter(|binding| matches!(binding.host, AdapterHostBinding::IsolatedWorker { .. }))
            .flat_map(|binding| {
                binding.features.iter().copied().map(|feature| {
                    (
                        BoundFeature::new(binding.adapter_id.clone(), binding.version, feature),
                        publication.generation(),
                    )
                })
            });
        Ok(HostGenerationReport::reported(
            placements
                .target_process
                .then_some(publication.generation()),
            isolated,
        ))
    }

    fn control_capture(
        &mut self,
        session_id: SessionId,
        target: &TargetInstance,
        paused: bool,
    ) -> Result<(), HostFailure> {
        let placements = self
            .active
            .get(target.id())
            .copied()
            .ok_or(HostFailure::Unavailable)?;
        if paused {
            if placements.target_process {
                self.target_process
                    .control_capture(session_id, target, true)?;
            }
            if placements.isolated_worker
                && self
                    .isolated_worker
                    .as_mut()
                    .ok_or(HostFailure::Unavailable)?
                    .control_capture(session_id, target, true)
                    .is_err()
            {
                if placements.target_process {
                    let _ = self
                        .target_process
                        .control_capture(session_id, target, false);
                }
                return Err(HostFailure::Unavailable);
            }
        } else {
            if placements.isolated_worker {
                self.isolated_worker
                    .as_mut()
                    .ok_or(HostFailure::Unavailable)?
                    .control_capture(session_id, target, false)?;
            }
            if placements.target_process
                && self
                    .target_process
                    .control_capture(session_id, target, false)
                    .is_err()
            {
                if placements.isolated_worker {
                    let _ = self
                        .isolated_worker
                        .as_mut()
                        .and_then(|host| host.control_capture(session_id, target, true).ok());
                }
                return Err(HostFailure::Unavailable);
            }
        }
        Ok(())
    }

    fn control_runtime_diagnostics(
        &mut self,
        session_id: SessionId,
        target: &TargetInstance,
        enabled: bool,
    ) -> Result<(), HostFailure> {
        let placements = self
            .active
            .get(target.id())
            .copied()
            .ok_or(HostFailure::Unavailable)?;
        if !placements.target_process {
            return Err(HostFailure::Unavailable);
        }
        self.target_process
            .control_runtime_diagnostics(session_id, target, enabled)
    }

    fn query_runtime_diagnostics(
        &mut self,
        session_id: SessionId,
        target: &TargetInstance,
    ) -> Result<glyphshift_session::RuntimeTraceBatch, HostFailure> {
        self.target_process
            .query_runtime_diagnostics(session_id, target)
    }

    fn health(
        &mut self,
        session_id: SessionId,
        target: &TargetInstance,
        bindings: &[AdapterBinding],
    ) -> Result<HostHealthReport, HostFailure> {
        let placements = self
            .active
            .get(target.id())
            .copied()
            .ok_or(HostFailure::Unavailable)?;
        let mut report = HostHealthReport::healthy();
        if placements.target_process {
            report = report.merge(self.target_process.health(session_id, target, bindings)?);
        }
        if placements.isolated_worker {
            report = report.merge(
                self.isolated_worker
                    .as_mut()
                    .ok_or(HostFailure::Unavailable)?
                    .health(session_id, target, bindings)?,
            );
        }
        Ok(report)
    }

    fn deactivate(
        &mut self,
        session_id: SessionId,
        target: &TargetInstance,
        bindings: &[AdapterBinding],
        plan: &[AdapterDeactivation],
    ) -> Result<HostDeactivation, HostFailure> {
        let placements = self
            .active
            .remove(target.id())
            .ok_or(HostFailure::Unavailable)?;
        let mut report = HostDeactivation::pending();
        if placements.target_process {
            report = report.merge(
                self.target_process
                    .deactivate(session_id, target, bindings, plan)
                    .unwrap_or_else(|_| {
                        HostDeactivation::reported(
                            [],
                            Self::adapters_for(bindings, |host| {
                                matches!(host, AdapterHostBinding::TargetProcess { .. })
                            }),
                        )
                    }),
            );
        }
        if placements.isolated_worker {
            report = report.merge(
                self.isolated_worker
                    .as_mut()
                    .ok_or(HostFailure::Unavailable)?
                    .deactivate(session_id, target, bindings, plan)
                    .unwrap_or_else(|_| {
                        HostDeactivation::reported(
                            [],
                            Self::adapters_for(bindings, |host| {
                                matches!(host, AdapterHostBinding::IsolatedWorker { .. })
                            }),
                        )
                    }),
            );
        }
        Ok(report)
    }

    fn release(
        &mut self,
        session_id: SessionId,
        target: &TargetInstance,
        bindings: &[AdapterBinding],
    ) -> Result<(), HostFailure> {
        let placements = self.active.remove(target.id()).unwrap_or(ActivePlacements {
            target_process: true,
            isolated_worker: self.isolated_worker.is_some(),
        });
        if placements.target_process {
            self.target_process.release(session_id, target, bindings)?;
        }
        if placements.isolated_worker {
            self.isolated_worker
                .as_mut()
                .ok_or(HostFailure::Unavailable)?
                .release(session_id, target, bindings)?;
        }
        Ok(())
    }
}

fn decode_health(report: WireWorkerHealthReport) -> Result<WorkerHealthReport, WorkerHostError> {
    if report
        .code
        .as_ref()
        .is_some_and(|code| !valid_worker_code(code))
    {
        return Err(WorkerHostError::MalformedMessage);
    }
    let state = match report.state {
        WireWorkerHealth::Healthy if report.code.is_none() => WorkerHealth::Healthy,
        WireWorkerHealth::Degraded if report.code.is_some() => WorkerHealth::Degraded,
        _ => return Err(WorkerHostError::MalformedMessage),
    };
    Ok(WorkerHealthReport {
        state,
        code: report.code.map(Into::into),
    })
}

fn valid_worker_code(code: &str) -> bool {
    !code.is_empty()
        && code.len() <= MAX_HEALTH_CODE_BYTES
        && code
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
}

struct WorkerProcess {
    child: Child,
    input: Option<ChildStdin>,
    responses: Receiver<String>,
    reader: Option<JoinHandle<()>>,
    timeout: Duration,
    next_request_id: u64,
    terminated: bool,
}

impl WorkerProcess {
    fn spawn(executable: &Path, timeout: Duration) -> Result<Self, WorkerHostError> {
        let mut command = Command::new(executable);
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        hide_window(&mut command);
        let mut child = command.spawn().map_err(|_| WorkerHostError::SpawnFailed)?;
        let input = child.stdin.take().ok_or(WorkerHostError::SpawnFailed)?;
        let output = child.stdout.take().ok_or(WorkerHostError::SpawnFailed)?;
        let (sender, responses) = mpsc::channel();
        let reader = thread::Builder::new()
            .name("glyphshift-isolated-worker-reader".into())
            .spawn(move || {
                for line in BufReader::new(output).lines() {
                    let Ok(line) = line else {
                        break;
                    };
                    if sender.send(line).is_err() {
                        break;
                    }
                }
            })
            .map_err(|_| WorkerHostError::SpawnFailed)?;
        Ok(Self {
            child,
            input: Some(input),
            responses,
            reader: Some(reader),
            timeout,
            next_request_id: 0,
            terminated: false,
        })
    }

    fn round_trip(&mut self, request: Request) -> Result<Response, WorkerHostError> {
        if self.terminated {
            return Err(WorkerHostError::Crashed);
        }
        self.next_request_id = self.next_request_id.saturating_add(1);
        let request_id = self.next_request_id;
        let input = self.input.as_mut().ok_or(WorkerHostError::Crashed)?;
        serde_json::to_writer(
            &mut *input,
            &RequestEnvelope {
                schema: PROTOCOL_SCHEMA.into(),
                request_id,
                request,
            },
        )
        .map_err(|_| WorkerHostError::MalformedMessage)?;
        input
            .write_all(b"\n")
            .and_then(|()| input.flush())
            .map_err(|_| WorkerHostError::Crashed)?;
        let line = match self.responses.recv_timeout(self.timeout) {
            Ok(line) => line,
            Err(error) => {
                let error = match error {
                    mpsc::RecvTimeoutError::Timeout => WorkerHostError::Timeout,
                    mpsc::RecvTimeoutError::Disconnected => WorkerHostError::Crashed,
                };
                self.terminate();
                return Err(error);
            }
        };
        let response: ResponseEnvelope = match serde_json::from_str(&line) {
            Ok(response) => response,
            Err(_) => {
                self.terminate();
                return Err(WorkerHostError::MalformedMessage);
            }
        };
        if response.schema != PROTOCOL_SCHEMA || response.request_id != request_id {
            self.terminate();
            return Err(WorkerHostError::MalformedMessage);
        }
        match response.response {
            Response::Error { code } if valid_worker_code(&code) => {
                Err(WorkerHostError::WorkerRejected(code.into()))
            }
            Response::Error { .. } => {
                self.terminate();
                Err(WorkerHostError::MalformedMessage)
            }
            response => Ok(response),
        }
    }

    fn terminate(&mut self) {
        if self.terminated {
            return;
        }
        self.terminated = true;
        if let Some(mut input) = self.input.take() {
            self.next_request_id = self.next_request_id.saturating_add(1);
            let _ = serde_json::to_writer(
                &mut input,
                &RequestEnvelope {
                    schema: PROTOCOL_SCHEMA.into(),
                    request_id: self.next_request_id,
                    request: Request::Terminate,
                },
            );
            let _ = input.write_all(b"\n");
            let _ = input.flush();
        }
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}

#[cfg(windows)]
fn hide_window(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn hide_window(_command: &mut Command) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_permission_and_timeout_have_stable_host_operation_failures() {
        assert_eq!(
            map_worker_error(WorkerHostError::WorkerRejected(
                "uia_permission_denied".into()
            )),
            HostFailure::OperationRejected(HostOperationFailure::IsolatedWorkerPermissionDenied)
        );
        assert_eq!(
            map_worker_error(WorkerHostError::Timeout),
            HostFailure::OperationRejected(HostOperationFailure::IsolatedWorkerTimeout)
        );
    }

    #[test]
    fn worker_codes_are_bounded_before_crossing_the_host_boundary() {
        assert!(valid_worker_code("uia_permission_denied"));
        assert!(!valid_worker_code("permission denied"));
        assert!(!valid_worker_code(""));
        assert!(!valid_worker_code(&"x".repeat(MAX_HEALTH_CODE_BYTES + 1)));
    }
}

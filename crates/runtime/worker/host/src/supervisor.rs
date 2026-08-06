use crate::{
    ProcessIsolatedWorker, WorkerArtifact, WorkerDrainReport, WorkerHealth, WorkerHealthReport,
    WorkerHostError,
};
use glyphshift_capture::{CaptureIngress, CaptureProducerConfiguration};
use glyphshift_domain::AdapterId;
use glyphshift_isolated_worker_sdk::WorkerTargetGrant;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, RecvTimeoutError, SyncSender};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const WORKER_POLL_INTERVAL: Duration = Duration::from_millis(250);
const WORKER_RESTART_WINDOW: Duration = Duration::from_secs(60);
const MAX_WORKER_RESTARTS: usize = 3;
const SUPERVISOR_RUNNING: u64 = 0;
const SUPERVISOR_RESTART_EXHAUSTED: u64 = 1;
const SUPERVISOR_RESTART_FAILED: u64 = 2;

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

pub(super) struct WorkerSupervisor {
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
    pub(super) fn start(
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

    pub(super) fn set_paused(&self, paused: bool) -> Result<(), WorkerHostError> {
        let (reply, response) = sync_channel(1);
        self.commands
            .send(WorkerSupervisorCommand::SetPaused { paused, reply })
            .map_err(|_| WorkerHostError::Crashed)?;
        response.recv().map_err(|_| WorkerHostError::Crashed)?
    }

    pub(super) fn update_generation(&self, generation: u64) -> Result<u64, WorkerHostError> {
        let (reply, response) = sync_channel(1);
        self.commands
            .send(WorkerSupervisorCommand::UpdateGeneration { generation, reply })
            .map_err(|_| WorkerHostError::Crashed)?;
        response.recv().map_err(|_| WorkerHostError::Crashed)?
    }

    pub(super) fn health(&self) -> Result<WorkerHealthReport, WorkerHostError> {
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

    pub(super) fn finish(&mut self) -> Result<WorkerDrainReport, WorkerHostError> {
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

pub(super) fn allocate_producer_generation(allocator: &AtomicU64) -> Result<u64, WorkerHostError> {
    allocator
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |generation| {
            generation.checked_add(1)
        })
        .map_err(|_| WorkerHostError::SpawnFailed)
}

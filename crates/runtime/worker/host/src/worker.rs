use glyphshift_adapter_registry::PackageArtifactId;
use glyphshift_capture::{
    CaptureIngress, CaptureObservationBatch, CaptureObservationCursor, CaptureProducerConfiguration,
};
use glyphshift_domain::AdapterId;
use glyphshift_isolated_worker_sdk::{
    Request, RequestEnvelope, Response, ResponseEnvelope, WireWorkerHealth, WireWorkerHealthReport,
    WorkerActivation, WorkerTargetGrant, PROTOCOL_SCHEMA,
};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread::{self, JoinHandle};
use std::time::Duration;

const MAX_DRAIN_BATCHES: usize = 64;
pub(super) const MAX_HEALTH_CODE_BYTES: usize = 128;
const MAX_TARGET_GRANT_BYTES: usize = 4_096;

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

    pub(super) fn artifact(&self, artifact_id: &PackageArtifactId) -> Option<WorkerArtifact> {
        self.workers.get(artifact_id).cloned()
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
    pub(super) state: WorkerHealth,
    pub(super) code: Option<Box<str>>,
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
            let _ = ingress.try_observe_with_context(
                record.adapter_id(),
                record.source(),
                record.translation_context().cloned(),
            );
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

pub(super) fn valid_worker_code(code: &str) -> bool {
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

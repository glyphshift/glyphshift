//! Supervised one-shot host for interactive text acquisition workers.

use glyphshift_acquisition::{
    AcquisitionError, AcquisitionRequest, AcquisitionResult, AuthorizedTarget,
};
use glyphshift_acquisition_worker_sdk::{
    decode_response, encode_request, WireError, WorkerTargetGrant, MAX_WIRE_BYTES,
};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const REQUEST_ID: u64 = 1;
const CANCEL_POLL_INTERVAL: Duration = Duration::from_millis(10);
const MAX_ADAPTER_ID_BYTES: usize = 256;

#[derive(Clone)]
pub struct AcquisitionWorkerArtifact {
    executable: PathBuf,
}

impl AcquisitionWorkerArtifact {
    pub fn open(executable: impl Into<PathBuf>) -> Result<Self, AcquisitionWorkerHostError> {
        let executable = executable.into();
        if !executable.is_absolute() || !executable.is_file() {
            return Err(AcquisitionWorkerHostError::ArtifactUnavailable);
        }
        Ok(Self { executable })
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct AcquisitionWorkerBinding {
    target: AuthorizedTarget,
    adapter_id: Box<str>,
    target_grant: WorkerTargetGrant,
}

impl AcquisitionWorkerBinding {
    pub fn new(
        target: AuthorizedTarget,
        adapter_id: impl Into<Box<str>>,
        grant_platform: impl Into<Box<str>>,
        grant_payload: impl Into<Box<str>>,
    ) -> Result<Self, AcquisitionWorkerHostError> {
        let adapter_id = adapter_id.into();
        if !valid_adapter_id(&adapter_id) {
            return Err(AcquisitionWorkerHostError::InvalidBinding);
        }
        let target_grant = WorkerTargetGrant::new(grant_platform, grant_payload)
            .map_err(|_| AcquisitionWorkerHostError::InvalidBinding)?;
        Ok(Self {
            target,
            adapter_id,
            target_grant,
        })
    }
}

#[derive(Clone, Default)]
pub struct CancellationToken(Arc<AtomicBool>);

impl CancellationToken {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

pub struct AcquisitionWorkerHost {
    artifact: AcquisitionWorkerArtifact,
    timeout: Duration,
}

impl AcquisitionWorkerHost {
    pub fn new(
        artifact: AcquisitionWorkerArtifact,
        timeout: Duration,
    ) -> Result<Self, AcquisitionWorkerHostError> {
        if timeout.is_zero() {
            return Err(AcquisitionWorkerHostError::InvalidBinding);
        }
        Ok(Self { artifact, timeout })
    }

    pub fn acquire(
        &self,
        binding: &AcquisitionWorkerBinding,
        request: &AcquisitionRequest,
        cancellation: &CancellationToken,
    ) -> Result<AcquisitionResult, AcquisitionWorkerHostError> {
        if request.target() != &binding.target {
            return Err(AcquisitionWorkerHostError::AcquisitionRejected(
                AcquisitionError::TargetMismatch,
            ));
        }
        if cancellation.is_cancelled() {
            return Err(AcquisitionWorkerHostError::Cancelled);
        }
        let encoded = encode_request(
            REQUEST_ID,
            &binding.adapter_id,
            &binding.target_grant,
            request,
        )
        .map_err(map_wire_error)?;
        let response = execute(
            &self.artifact.executable,
            &encoded,
            self.timeout,
            cancellation,
        )?;
        match decode_response(&response, REQUEST_ID, request).map_err(map_wire_error)? {
            Ok(result) => Ok(result),
            Err(error) => Err(AcquisitionWorkerHostError::AcquisitionRejected(error)),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AcquisitionWorkerHostError {
    ArtifactUnavailable,
    InvalidBinding,
    SpawnFailed,
    Crashed,
    Timeout,
    Cancelled,
    MalformedMessage,
    AcquisitionRejected(AcquisitionError),
}

fn execute(
    executable: &Path,
    request: &str,
    timeout: Duration,
    cancellation: &CancellationToken,
) -> Result<String, AcquisitionWorkerHostError> {
    let mut command = Command::new(executable);
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    hide_window(&mut command);
    let child = command
        .spawn()
        .map_err(|_| AcquisitionWorkerHostError::SpawnFailed)?;
    let mut process = ProcessGuard::new(child);
    let mut input = process
        .child
        .stdin
        .take()
        .ok_or(AcquisitionWorkerHostError::SpawnFailed)?;
    let output = process
        .child
        .stdout
        .take()
        .ok_or(AcquisitionWorkerHostError::SpawnFailed)?;
    let (sender, responses) = mpsc::channel();
    let reader = thread::Builder::new()
        .name("glyphshift-acquisition-worker-reader".into())
        .spawn(move || {
            let _ = sender.send(read_response(output));
        })
        .map_err(|_| AcquisitionWorkerHostError::SpawnFailed)?;
    process.reader = Some(reader);
    if input
        .write_all(request.as_bytes())
        .and_then(|()| input.write_all(b"\n"))
        .and_then(|()| input.flush())
        .is_err()
    {
        return Err(AcquisitionWorkerHostError::Crashed);
    }
    drop(input);
    receive_response(&mut process, responses, timeout, cancellation)
}

fn receive_response(
    process: &mut ProcessGuard,
    responses: Receiver<Result<String, AcquisitionWorkerHostError>>,
    timeout: Duration,
    cancellation: &CancellationToken,
) -> Result<String, AcquisitionWorkerHostError> {
    let deadline = Instant::now()
        .checked_add(timeout)
        .ok_or(AcquisitionWorkerHostError::Timeout)?;
    loop {
        if cancellation.is_cancelled() {
            process.terminate();
            return Err(AcquisitionWorkerHostError::Cancelled);
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            process.terminate();
            return Err(AcquisitionWorkerHostError::Timeout);
        }
        match responses.recv_timeout(remaining.min(CANCEL_POLL_INTERVAL)) {
            Ok(response) => return response,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err(AcquisitionWorkerHostError::Crashed);
            }
        }
    }
}

fn read_response(output: impl Read) -> Result<String, AcquisitionWorkerHostError> {
    let mut bytes = Vec::new();
    let read = BufReader::new(output)
        .take((MAX_WIRE_BYTES + 2) as u64)
        .read_until(b'\n', &mut bytes)
        .map_err(|_| AcquisitionWorkerHostError::Crashed)?;
    if read == 0 {
        return Err(AcquisitionWorkerHostError::Crashed);
    }
    if !bytes.ends_with(b"\n") {
        return Err(AcquisitionWorkerHostError::MalformedMessage);
    }
    bytes.pop();
    if bytes.len() > MAX_WIRE_BYTES {
        return Err(AcquisitionWorkerHostError::MalformedMessage);
    }
    String::from_utf8(bytes).map_err(|_| AcquisitionWorkerHostError::MalformedMessage)
}

struct ProcessGuard {
    child: Child,
    reader: Option<JoinHandle<()>>,
    terminated: bool,
}

impl ProcessGuard {
    const fn new(child: Child) -> Self {
        Self {
            child,
            reader: None,
            terminated: false,
        }
    }

    fn terminate(&mut self) {
        if self.terminated {
            return;
        }
        self.terminated = true;
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}

impl Drop for ProcessGuard {
    fn drop(&mut self) {
        self.terminate();
    }
}

fn map_wire_error(_error: WireError) -> AcquisitionWorkerHostError {
    AcquisitionWorkerHostError::MalformedMessage
}

fn valid_adapter_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_ADAPTER_ID_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
}

#[cfg(windows)]
fn hide_window(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn hide_window(_command: &mut Command) {}

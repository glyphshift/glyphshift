#![cfg(windows)]

use glyphshift_adapter_uia::ADAPTER_ID;
use glyphshift_capture::{
    CaptureCatalog, CaptureConfiguration, CaptureProducerConfiguration, CaptureProducerId,
    CaptureSessionId, FileCaptureSink,
};
use glyphshift_domain::AdapterId;
use glyphshift_isolated_worker_host::{ProcessIsolatedWorker, WorkerArtifact};
use glyphshift_isolated_worker_sdk::WorkerTargetGrant;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread::JoinHandle;
use std::time::Duration;
use tempfile::tempdir;
use windows_sys::Win32::Foundation::{CloseHandle, FILETIME};
use windows_sys::Win32::Security::{
    GetSidSubAuthority, GetSidSubAuthorityCount, GetTokenInformation, TokenIntegrityLevel,
    TOKEN_MANDATORY_LABEL, TOKEN_QUERY,
};
use windows_sys::Win32::System::Threading::{
    GetProcessTimes, OpenProcess, OpenProcessToken, PROCESS_QUERY_LIMITED_INFORMATION,
};

struct StandardControlTarget {
    child: Child,
    stdin: ChildStdin,
    responses: Receiver<String>,
    reader: Option<JoinHandle<()>>,
}

impl StandardControlTarget {
    fn start() -> Self {
        use std::os::windows::process::CommandExt;

        let mut child = Command::new(runtime_target_executable())
            .arg("--uia-standard-controls")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .creation_flags(0x0800_0000)
            .spawn()
            .expect("start standard control target");
        let stdin = child.stdin.take().expect("target stdin");
        let stdout = child.stdout.take().expect("target stdout");
        let (sender, responses) = mpsc::channel();
        let reader = std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else {
                    break;
                };
                if sender.send(line).is_err() {
                    break;
                }
            }
        });
        let target = Self {
            child,
            stdin,
            responses,
            reader: Some(reader),
        };
        assert_eq!(target.read_line(Duration::from_secs(3)), "uia-ready");
        target
    }

    fn process_grant(&self) -> WorkerTargetGrant {
        let process_id = self.child.id();
        let started_at = process_started_at(process_id).expect("target start time");
        WorkerTargetGrant {
            platform: "windows-process-v1".into(),
            payload: format!("{process_id}:{started_at}"),
        }
    }

    fn process_id(&self) -> u32 {
        self.child.id()
    }

    fn update(&mut self) {
        writeln!(self.stdin, "update").expect("update target");
        self.stdin.flush().expect("flush target update");
        assert_eq!(self.read_line(Duration::from_secs(3)), "uia-updated");
    }

    fn begin_recreate(&mut self) {
        writeln!(self.stdin, "recreate").expect("recreate target");
        self.stdin.flush().expect("flush target recreation");
    }

    fn wait_for_recreation(&self) {
        assert_eq!(self.read_line(Duration::from_secs(3)), "uia-recreated");
    }

    fn begin_provider_block(&mut self) {
        writeln!(self.stdin, "block-provider").expect("block target UIA provider");
        self.stdin.flush().expect("flush target provider block");
        assert_eq!(
            self.read_line(Duration::from_secs(3)),
            "uia-provider-blocking"
        );
    }

    fn unblock_provider(&mut self) {
        writeln!(self.stdin, "unblock-provider").expect("unblock target UIA provider");
        self.stdin.flush().expect("flush target provider unblock");
        assert_eq!(
            self.read_line(Duration::from_secs(3)),
            "uia-provider-unblocked"
        );
    }

    fn stop(mut self) {
        writeln!(self.stdin, "exit").expect("stop target");
        self.stdin.flush().expect("flush target exit");
        assert_eq!(self.read_line(Duration::from_secs(3)), "uia-exiting");
        for _ in 0..30 {
            if let Some(status) = self.child.try_wait().expect("query target exit") {
                assert!(status.success());
                self.join_reader();
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
        self.join_reader();
        panic!("standard control target did not exit in time");
    }

    fn read_line(&self, timeout: Duration) -> String {
        self.responses
            .recv_timeout(timeout)
            .expect("target response within timeout")
            .trim()
            .to_owned()
    }

    fn join_reader(&mut self) {
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}

impl Drop for StandardControlTarget {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
        self.join_reader();
    }
}

fn runtime_target_executable() -> PathBuf {
    let executable = std::env::current_exe().expect("current test executable");
    let profile = executable
        .parent()
        .and_then(|deps| deps.parent())
        .expect("Cargo profile directory");
    profile.join(format!(
        "glyphshift-windows-runtime-target{}",
        std::env::consts::EXE_SUFFIX
    ))
}

fn process_started_at(process_id: u32) -> Option<u64> {
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, process_id) };
    if process.is_null() {
        return None;
    }
    let mut created = FILETIME::default();
    let mut exited = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    let queried =
        unsafe { GetProcessTimes(process, &mut created, &mut exited, &mut kernel, &mut user) } != 0;
    unsafe { CloseHandle(process) };
    queried.then_some(((created.dwHighDateTime as u64) << 32) | created.dwLowDateTime as u64)
}

fn process_integrity_rid(process_id: u32) -> Option<u32> {
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, process_id) };
    if process.is_null() {
        return None;
    }
    let mut token = std::ptr::null_mut();
    if unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut token) } == 0 {
        unsafe { CloseHandle(process) };
        return None;
    }
    let mut required = 0;
    unsafe {
        GetTokenInformation(
            token,
            TokenIntegrityLevel,
            std::ptr::null_mut(),
            0,
            &mut required,
        );
    }
    let mut buffer = vec![0_u8; required as usize];
    let queried = required > 0
        && unsafe {
            GetTokenInformation(
                token,
                TokenIntegrityLevel,
                buffer.as_mut_ptr().cast(),
                required,
                &mut required,
            )
        } != 0;
    let integrity = queried.then(|| unsafe {
        let label = &*buffer.as_ptr().cast::<TOKEN_MANDATORY_LABEL>();
        let count = *GetSidSubAuthorityCount(label.Label.Sid) as u32;
        *GetSidSubAuthority(label.Label.Sid, count.saturating_sub(1))
    });
    unsafe {
        CloseHandle(token);
        CloseHandle(process);
    }
    integrity
}

fn spawn_worker(
    target: &StandardControlTarget,
    artifact: &WorkerArtifact,
    producer_generation: u64,
    timeout: Duration,
) -> ProcessIsolatedWorker {
    ProcessIsolatedWorker::spawn(
        artifact.clone(),
        timeout,
        &AdapterId::new(ADAPTER_ID),
        target.process_grant(),
        CaptureProducerConfiguration::new(
            CaptureProducerId::new(format!("windows-uia-worker-{producer_generation}"))
                .expect("producer id"),
            producer_generation,
        )
        .expect("producer configuration"),
        1,
    )
    .expect("activate Windows UIA worker")
}

#[test]
fn uia_worker_observes_standard_controls_and_rejects_password_text() {
    let root = tempdir().expect("temporary capture root");
    let output = root.path().join("windows-uia-capture.json");
    let sink = FileCaptureSink::start(
        CaptureConfiguration::new(
            CaptureSessionId::new("windows-uia-contract").expect("session id"),
            &output,
            100,
        )
        .expect("capture configuration"),
    )
    .expect("capture owner");
    let mut target = StandardControlTarget::start();
    assert_eq!(
        process_integrity_rid(target.process_id()),
        process_integrity_rid(std::process::id()),
        "standard UIA contract must exercise a same-integrity target"
    );
    std::thread::sleep(Duration::from_millis(150));
    let artifact = WorkerArtifact::open(PathBuf::from(env!(
        "CARGO_BIN_EXE_glyphshift-adapter-uia-worker"
    )))
    .expect("UIA worker artifact");
    let mut worker = spawn_worker(&target, &artifact, 1, Duration::from_secs(5));
    worker
        .drain_until_empty(&sink.ingress())
        .expect("initial UIA observations");

    target.update();
    let mut updated_records = 0;
    for _ in 0..20 {
        std::thread::sleep(Duration::from_millis(100));
        updated_records += worker
            .drain_until_empty(&sink.ingress())
            .expect("changed UIA observations")
            .records();
    }
    assert!(updated_records > 0, "UIA change event was not observed");
    worker
        .deactivate(&sink.ingress())
        .expect("remove UIA handlers and drain tail");
    sink.finish().expect("finish capture owner");
    target.stop();

    let catalog = CaptureCatalog::read_current(&output).expect("capture checkpoint");
    let sources = catalog
        .entries()
        .iter()
        .map(|entry| entry.source())
        .collect::<Vec<_>>();
    for expected in [
        "Fixture label",
        "Fixture value",
        "Fixture document",
        "Updated label",
        "Updated value",
        "Updated document",
    ] {
        assert!(
            sources.contains(&expected),
            "missing {expected}; sources={sources:?}"
        );
    }
    assert!(!sources.contains(&"Fixture secret"));
    assert!(!sources.contains(&"Updated secret"));
}

#[test]
fn uia_worker_recovers_after_the_target_recreates_its_window_tree() {
    let root = tempdir().expect("temporary capture root");
    let output = root.path().join("windows-uia-recreation.json");
    let sink = FileCaptureSink::start(
        CaptureConfiguration::new(
            CaptureSessionId::new("windows-uia-recreation").expect("session id"),
            &output,
            100,
        )
        .expect("capture configuration"),
    )
    .expect("capture owner");
    let mut target = StandardControlTarget::start();
    std::thread::sleep(Duration::from_millis(150));
    let artifact = WorkerArtifact::open(PathBuf::from(env!(
        "CARGO_BIN_EXE_glyphshift-adapter-uia-worker"
    )))
    .expect("UIA worker artifact");
    let mut worker = spawn_worker(&target, &artifact, 1, Duration::from_secs(3));
    worker
        .drain_until_empty(&sink.ingress())
        .expect("initial UIA observations");

    target.begin_recreate();
    let recreation_query = worker.drain_once(&sink.ingress());
    target.wait_for_recreation();
    if recreation_query.is_err() {
        drop(worker);
        worker = spawn_worker(&target, &artifact, 2, Duration::from_secs(3));
    }
    for _ in 0..20 {
        std::thread::sleep(Duration::from_millis(100));
        worker
            .drain_until_empty(&sink.ingress())
            .expect("recreated UIA observations");
    }
    worker
        .deactivate(&sink.ingress())
        .expect("deactivate recreated UIA worker");
    sink.finish().expect("finish capture owner");
    target.stop();

    let catalog = CaptureCatalog::read_current(&output).expect("capture checkpoint");
    let sources = catalog
        .entries()
        .iter()
        .map(|entry| entry.source())
        .collect::<Vec<_>>();
    for expected in ["Recreated label", "Recreated value", "Recreated document"] {
        assert!(
            sources.contains(&expected),
            "missing {expected}; sources={sources:?}"
        );
    }
    assert!(!sources.contains(&"Recreated secret"));
}

#[test]
fn uia_worker_timeout_releases_a_blocked_provider_and_reconnects() {
    let root = tempdir().expect("temporary capture root");
    let output = root.path().join("windows-uia-provider-block.json");
    let sink = FileCaptureSink::start(
        CaptureConfiguration::new(
            CaptureSessionId::new("windows-uia-provider-block").expect("session id"),
            &output,
            100,
        )
        .expect("capture configuration"),
    )
    .expect("capture owner");
    let mut target = StandardControlTarget::start();
    std::thread::sleep(Duration::from_millis(150));
    let artifact = WorkerArtifact::open(PathBuf::from(env!(
        "CARGO_BIN_EXE_glyphshift-adapter-uia-worker"
    )))
    .expect("UIA worker artifact");
    let mut worker = spawn_worker(&target, &artifact, 1, Duration::from_secs(2));
    worker
        .drain_until_empty(&sink.ingress())
        .expect("initial UIA observations");

    target.begin_provider_block();
    std::thread::sleep(Duration::from_millis(1_100));
    assert_eq!(
        worker.drain_once(&sink.ingress()),
        Err(glyphshift_isolated_worker_host::WorkerHostError::Timeout)
    );
    assert_eq!(
        worker.health(),
        Err(glyphshift_isolated_worker_host::WorkerHostError::Crashed)
    );

    target.unblock_provider();
    let mut replacement = spawn_worker(&target, &artifact, 2, Duration::from_secs(3));
    let recovered = replacement
        .drain_until_empty(&sink.ingress())
        .expect("reconnected UIA observations");
    assert!(recovered.records() > 0);
    replacement
        .deactivate(&sink.ingress())
        .expect("deactivate replacement UIA worker");
    sink.finish().expect("finish capture owner");
    target.stop();
}

#[test]
#[ignore = "requires an explicitly authorized running Windows target process"]
fn authorized_windows_target_exposes_useful_uia_text_without_modification() {
    let process_id = std::env::var("GLYPHSHIFT_UIA_TARGET_PID")
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .filter(|value| *value > 0)
        .expect("authorized UIA target process environment variable");
    let started_at = process_started_at(process_id).expect("authorized target start time");
    let evidence_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../target/local-test/evidence/uia-authorized-smoke");
    fs::create_dir_all(&evidence_root).expect("create local UIA evidence root");
    let output = evidence_root.join("capture.json");
    if output.exists() {
        fs::remove_file(&output).expect("replace prior local UIA smoke evidence");
    }
    let sink = FileCaptureSink::start(
        CaptureConfiguration::new(
            CaptureSessionId::new("uia-authorized-smoke").expect("session id"),
            &output,
            5_000,
        )
        .expect("capture configuration"),
    )
    .expect("capture owner");
    let artifact = WorkerArtifact::open(PathBuf::from(env!(
        "CARGO_BIN_EXE_glyphshift-adapter-uia-worker"
    )))
    .expect("UIA worker artifact");
    let mut worker = ProcessIsolatedWorker::spawn(
        artifact,
        Duration::from_secs(5),
        &AdapterId::new(ADAPTER_ID),
        WorkerTargetGrant {
            platform: "windows-process-v1".into(),
            payload: format!("{process_id}:{started_at}"),
        },
        CaptureProducerConfiguration::new(
            CaptureProducerId::new("uia-authorized-smoke").expect("producer id"),
            1,
        )
        .expect("producer configuration"),
        1,
    )
    .expect("activate authorized Windows UIA worker");
    for _ in 0..20 {
        std::thread::sleep(Duration::from_millis(250));
        worker
            .drain_until_empty(&sink.ingress())
            .expect("drain authorized UIA observations");
    }
    let health = worker.health().expect("authorized UIA health");
    worker
        .deactivate(&sink.ingress())
        .expect("deactivate authorized UIA worker");
    sink.finish().expect("finish authorized UIA capture");

    let catalog = CaptureCatalog::read_current(&output).expect("authorized UIA catalog");
    eprintln!(
        "authorized UIA smoke: {} unique public texts; health={:?}; code={:?}",
        catalog.entries().len(),
        health.state(),
        health.code()
    );
    assert!(
        !catalog.entries().is_empty(),
        "authorized target exposed no useful UIA text"
    );
}

#[test]
#[ignore = "requires an explicitly authorized elevated synthetic target process"]
fn high_integrity_target_fails_closed_before_partial_observation() {
    let process_id = std::env::var("GLYPHSHIFT_UIA_HIGH_INTEGRITY_TARGET_PID")
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .filter(|value| *value > 0)
        .expect("authorized elevated UIA target process environment variable");
    let current_integrity = process_integrity_rid(std::process::id())
        .expect("current UIA test process integrity level");
    let target_integrity =
        process_integrity_rid(process_id).expect("elevated target integrity level");
    assert!(
        target_integrity > current_integrity,
        "target must run at a higher integrity level"
    );
    let started_at = process_started_at(process_id).expect("elevated target start time");
    let root = tempdir().expect("temporary capture root");
    let output = root.path().join("high-integrity-uia-capture.json");
    let sink = FileCaptureSink::start(
        CaptureConfiguration::new(
            CaptureSessionId::new("high-integrity-uia-contract").expect("session id"),
            &output,
            100,
        )
        .expect("capture configuration"),
    )
    .expect("capture owner");
    let artifact = WorkerArtifact::open(PathBuf::from(env!(
        "CARGO_BIN_EXE_glyphshift-adapter-uia-worker"
    )))
    .expect("UIA worker artifact");
    let result = ProcessIsolatedWorker::spawn(
        artifact,
        Duration::from_secs(5),
        &AdapterId::new(ADAPTER_ID),
        WorkerTargetGrant {
            platform: "windows-process-v1".into(),
            payload: format!("{process_id}:{started_at}"),
        },
        CaptureProducerConfiguration::new(
            CaptureProducerId::new("high-integrity-uia-contract").expect("producer id"),
            1,
        )
        .expect("producer configuration"),
        1,
    );

    match result {
        Err(glyphshift_isolated_worker_host::WorkerHostError::WorkerRejected(code)) => {
            assert_eq!(code.as_ref(), "uia_permission_denied");
        }
        Ok(mut worker) => {
            let _ = worker.deactivate(&sink.ingress());
            panic!("higher-integrity target must be rejected before partial UIA observation");
        }
        Err(error) => panic!("unexpected elevated target result: {error:?}"),
    }
    sink.finish().expect("finish capture owner");
    let catalog = CaptureCatalog::read_current(&output).expect("elevated target capture");
    assert!(catalog.entries().is_empty());
}

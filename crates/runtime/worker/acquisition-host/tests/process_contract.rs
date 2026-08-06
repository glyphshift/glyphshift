use glyphshift_acquisition::{
    AcquisitionError, AcquisitionRequest, AuthorizedTarget, DesktopPoint, InteractiveSelection,
    SourcePolicy,
};
use glyphshift_acquisition_worker_host::{
    AcquisitionWorkerArtifact, AcquisitionWorkerBinding, AcquisitionWorkerHost,
    AcquisitionWorkerHostError, CancellationToken,
};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tempfile::tempdir;

fn worker_executable(name: &str) -> PathBuf {
    let executable = std::env::current_exe().expect("current test executable");
    let profile = executable
        .parent()
        .and_then(|deps| deps.parent())
        .expect("Cargo profile directory");
    profile.join(format!("{name}{}", std::env::consts::EXE_SUFFIX))
}

fn target(value: &str) -> AuthorizedTarget {
    AuthorizedTarget::new(value).expect("authorized target")
}

fn request(value: &str) -> AcquisitionRequest {
    AcquisitionRequest::new(
        target(value),
        InteractiveSelection::Point(DesktopPoint::new(-20, 30)),
        SourcePolicy::StructuredOnly,
    )
}

fn binding(payload: &str) -> AcquisitionWorkerBinding {
    AcquisitionWorkerBinding::new(
        target("target-a"),
        "synthetic.acquisition",
        "synthetic-process-v1",
        payload,
    )
    .expect("worker binding")
}

fn host(name: &str, timeout: Duration) -> AcquisitionWorkerHost {
    let artifact =
        AcquisitionWorkerArtifact::open(worker_executable(name)).expect("worker artifact");
    AcquisitionWorkerHost::new(artifact, timeout).expect("worker host")
}

#[test]
fn acquisition_worker_001_returns_bounded_text_for_the_bound_target() {
    let result = host("glyphshift-test-acquisition-worker", Duration::from_secs(2))
        .acquire(
            &binding("authorized"),
            &request("target-a"),
            &CancellationToken::new(),
        )
        .expect("worker acquisition");

    assert_eq!(result.blocks().len(), 1);
    assert_eq!(result.blocks()[0].source(), "synthetic worker text");
    assert!(result.blocks()[0]
        .anchors()
        .iter()
        .any(|anchor| anchor.contains(DesktopPoint::new(-20, 30))));
}

#[test]
fn acquisition_worker_002_rejects_a_request_for_a_different_local_target_before_spawn() {
    let result = host("glyphshift-test-acquisition-worker", Duration::from_secs(2)).acquire(
        &binding("authorized"),
        &request("target-b"),
        &CancellationToken::new(),
    );

    assert_eq!(
        result,
        Err(AcquisitionWorkerHostError::AcquisitionRejected(
            AcquisitionError::TargetMismatch
        ))
    );
}

#[test]
fn acquisition_worker_003_preserves_stable_worker_rejections() {
    let host = host("glyphshift-test-acquisition-worker", Duration::from_secs(2));
    for (payload, expected) in [
        ("permission-denied", AcquisitionError::PermissionDenied),
        ("target-mismatch", AcquisitionError::TargetMismatch),
        ("no-text", AcquisitionError::NoText),
    ] {
        assert_eq!(
            host.acquire(
                &binding(payload),
                &request("target-a"),
                &CancellationToken::new(),
            ),
            Err(AcquisitionWorkerHostError::AcquisitionRejected(expected))
        );
    }
}

#[test]
fn acquisition_worker_004_timeout_and_cancellation_reclaim_a_hung_process() {
    let timeout_host = host(
        "glyphshift-test-acquisition-worker",
        Duration::from_millis(100),
    );
    let started = Instant::now();
    assert_eq!(
        timeout_host.acquire(
            &binding("hang"),
            &request("target-a"),
            &CancellationToken::new(),
        ),
        Err(AcquisitionWorkerHostError::Timeout)
    );
    assert!(started.elapsed() < Duration::from_secs(2));

    let cancellation = CancellationToken::new();
    let trigger = cancellation.clone();
    let canceller = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(50));
        trigger.cancel();
    });
    let cancel_host = host("glyphshift-test-acquisition-worker", Duration::from_secs(2));
    assert_eq!(
        cancel_host.acquire(&binding("hang"), &request("target-a"), &cancellation),
        Err(AcquisitionWorkerHostError::Cancelled)
    );
    canceller.join().expect("canceller");
}

#[test]
fn acquisition_worker_005_crash_and_malformed_response_are_distinct_host_failures() {
    assert_eq!(
        host("glyphshift-test-acquisition-worker", Duration::from_secs(2)).acquire(
            &binding("crash"),
            &request("target-a"),
            &CancellationToken::new(),
        ),
        Err(AcquisitionWorkerHostError::Crashed)
    );
    assert_eq!(
        host(
            "glyphshift-test-malformed-acquisition-worker",
            Duration::from_secs(2),
        )
        .acquire(
            &binding("authorized"),
            &request("target-a"),
            &CancellationToken::new(),
        ),
        Err(AcquisitionWorkerHostError::MalformedMessage)
    );
}

#[test]
fn acquisition_worker_006_requires_an_absolute_existing_artifact_and_valid_binding() {
    let root = tempdir().expect("temporary root");
    assert!(matches!(
        AcquisitionWorkerArtifact::open(root.path().join("missing-worker")),
        Err(AcquisitionWorkerHostError::ArtifactUnavailable)
    ));
    assert!(matches!(
        AcquisitionWorkerBinding::new(
            target("target-a"),
            "invalid adapter id",
            "synthetic-process-v1",
            "authorized",
        ),
        Err(AcquisitionWorkerHostError::InvalidBinding)
    ));
}

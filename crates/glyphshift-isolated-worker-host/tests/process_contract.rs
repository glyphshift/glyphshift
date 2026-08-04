use glyphshift_adapter_registry::{
    AdapterBinding, AdapterHostBinding, AdapterVersion, ArtifactHash, PackageArtifactId,
};
use glyphshift_adapter_sdk::AdapterDescriptor;
use glyphshift_capture::{
    CaptureCatalog, CaptureConfiguration, CaptureProducerConfiguration, CaptureProducerId,
    CaptureSessionId, FileCaptureSink,
};
use glyphshift_domain::{AdapterId, ApplyModel, Feature, Generation, Placement, TargetFacts};
use glyphshift_isolated_worker_host::{
    IsolatedWorkerHost, ProcessIsolatedWorker, WorkerArtifact, WorkerArtifactCatalog, WorkerHealth,
    WorkerHostError,
};
use glyphshift_isolated_worker_sdk::WorkerTargetGrant;
use glyphshift_session::{
    AdapterHostPort, HostFailure, HostOperationFailure, SessionId, TargetInstance, TargetInstanceId,
};
use std::path::PathBuf;
use std::time::Duration;
use tempfile::tempdir;

fn worker_executable() -> PathBuf {
    let executable = std::env::current_exe().expect("current test executable");
    let profile = executable
        .parent()
        .and_then(|deps| deps.parent())
        .expect("Cargo profile directory");
    profile.join(format!(
        "glyphshift-test-isolated-worker{}",
        std::env::consts::EXE_SUFFIX
    ))
}

fn producer() -> CaptureProducerConfiguration {
    CaptureProducerConfiguration::new(
        CaptureProducerId::new("worker-uia-1").expect("producer id"),
        7,
    )
    .expect("producer configuration")
}

fn target_grant(payload: &str) -> WorkerTargetGrant {
    WorkerTargetGrant {
        platform: "synthetic-process-v1".into(),
        payload: payload.into(),
    }
}

#[test]
fn iwh_001_worker_handshake_health_pause_and_deactivation_tail_use_one_capture_owner() {
    let root = tempdir().expect("temporary capture root");
    let output = root.path().join("isolated-worker-capture.json");
    let sink = FileCaptureSink::start(
        CaptureConfiguration::new(
            CaptureSessionId::new("isolated-worker-contract").expect("session id"),
            &output,
            100,
        )
        .expect("capture configuration"),
    )
    .expect("capture owner");
    let ingress = sink.ingress();
    let artifact = WorkerArtifact::open(worker_executable()).expect("worker artifact");
    let mut worker = ProcessIsolatedWorker::spawn(
        artifact,
        Duration::from_secs(2),
        &AdapterId::new("windows.uia.synthetic"),
        target_grant("target:authorized"),
        producer(),
        11,
    )
    .expect("worker handshake");

    let health = worker.health().expect("worker health");
    assert_eq!(health.state(), WorkerHealth::Healthy);
    assert_eq!(health.code(), None);
    assert_eq!(worker.update_generation(12), Ok(12));
    let initial = worker
        .drain_until_empty(&ingress)
        .expect("initial worker observations");
    assert_eq!(initial.records(), 3);

    worker.set_paused(true).expect("pause worker");
    assert_eq!(
        worker
            .drain_until_empty(&ingress)
            .expect("paused drain")
            .records(),
        0
    );
    worker.set_paused(false).expect("resume worker");
    assert_eq!(
        worker
            .drain_until_empty(&ingress)
            .expect("resumed drain")
            .records(),
        1
    );

    let deactivation = worker.deactivate(&ingress).expect("deactivate worker");
    assert_eq!(deactivation.records(), 1);
    sink.finish().expect("finish central capture owner");

    let catalog = CaptureCatalog::read_current(&output).expect("capture checkpoint");
    let sources = catalog
        .entries()
        .iter()
        .map(|entry| entry.source())
        .collect::<Vec<_>>();
    assert_eq!(catalog.entries().len(), 5);
    for expected in [
        "Window title",
        "Document text",
        "Field value",
        "Resumed label",
        "Handler removal tail",
    ] {
        assert!(sources.contains(&expected), "missing {expected}");
    }
}

#[test]
fn iwh_002_worker_rejects_an_unauthorized_target_during_handshake() {
    let artifact = WorkerArtifact::open(worker_executable()).expect("worker artifact");
    let result = ProcessIsolatedWorker::spawn(
        artifact,
        Duration::from_secs(2),
        &AdapterId::new("windows.uia.synthetic"),
        target_grant("target:not-authorized"),
        producer(),
        11,
    );
    assert!(matches!(
        result,
        Err(WorkerHostError::WorkerRejected(code)) if code.as_ref() == "activation_rejected"
    ));
}

#[test]
fn iwh_003_permission_rejection_reaches_the_generic_host_contract() {
    let root = tempdir().expect("temporary capture root");
    let output = root.path().join("isolated-worker-permission.json");
    let sink = FileCaptureSink::start(
        CaptureConfiguration::new(
            CaptureSessionId::new("isolated-worker-permission").expect("session id"),
            &output,
            100,
        )
        .expect("capture configuration"),
    )
    .expect("capture owner");
    let artifact_id = PackageArtifactId::new("workers/uia-permission");
    let artifacts = WorkerArtifactCatalog::new([(artifact_id.clone(), worker_executable())])
        .expect("worker catalog");
    let mut host = IsolatedWorkerHost::new(artifacts, sink.ingress(), Duration::from_secs(2));
    let target = TargetInstance::new(
        TargetInstanceId::new("target-permission"),
        TargetFacts::new("windows", "x86_64"),
    );
    host.register_target(
        target.id().clone(),
        target_grant("target:permission-denied"),
    );
    let adapter_id = AdapterId::new("windows.uia.synthetic");
    let version = AdapterVersion::new(1, 0, 0);
    let binding = AdapterBinding {
        descriptor: AdapterDescriptor::new(
            adapter_id.clone(),
            version,
            ApplyModel::ObserveOnly,
            Placement::IsolatedWorker,
            [Feature::TextObserve],
        ),
        adapter_id,
        version,
        apply_model: ApplyModel::ObserveOnly,
        artifact_hash: ArtifactHash::sha256([8; 32]),
        host: AdapterHostBinding::IsolatedWorker {
            executable: artifact_id,
        },
        features: vec![Feature::TextObserve],
    };

    assert_eq!(
        host.activate(&target, &[binding]),
        Err(HostFailure::OperationRejected(
            HostOperationFailure::IsolatedWorkerPermissionDenied
        ))
    );
    sink.finish().expect("finish capture owner");
}

#[test]
fn iwh_004_adapter_host_supervises_generation_capture_health_and_stop() {
    let root = tempdir().expect("temporary capture root");
    let output = root.path().join("isolated-adapter-host.json");
    let sink = FileCaptureSink::start(
        CaptureConfiguration::new(
            CaptureSessionId::new("isolated-adapter-host").expect("session id"),
            &output,
            100,
        )
        .expect("capture configuration"),
    )
    .expect("capture owner");
    let artifact_id = PackageArtifactId::new("workers/uia-synthetic");
    let artifacts = WorkerArtifactCatalog::new([(artifact_id.clone(), worker_executable())])
        .expect("worker catalog");
    let mut host = IsolatedWorkerHost::new(artifacts, sink.ingress(), Duration::from_secs(2));
    let target = TargetInstance::new(
        TargetInstanceId::new("target-authorized"),
        TargetFacts::new("windows", "x86_64"),
    );
    host.register_target(target.id().clone(), target_grant("target:authorized"));
    let adapter_id = AdapterId::new("windows.uia.synthetic");
    let version = AdapterVersion::new(1, 0, 0);
    let binding = AdapterBinding {
        descriptor: AdapterDescriptor::new(
            adapter_id.clone(),
            version,
            ApplyModel::ObserveOnly,
            Placement::IsolatedWorker,
            [Feature::TextObserve],
        ),
        adapter_id,
        version,
        apply_model: ApplyModel::ObserveOnly,
        artifact_hash: ArtifactHash::sha256([7; 32]),
        host: AdapterHostBinding::IsolatedWorker {
            executable: artifact_id,
        },
        features: vec![Feature::TextObserve],
    };

    host.activate(&target, std::slice::from_ref(&binding))
        .expect("activate isolated binding");
    host.update(
        SessionId::new(1),
        &target,
        std::slice::from_ref(&binding),
        Generation::new(2),
    )
    .expect("acknowledge publication generation");
    host.health(SessionId::new(1), &target, std::slice::from_ref(&binding))
        .expect("worker health");
    host.control_capture(SessionId::new(1), &target, true)
        .expect("pause worker");
    host.control_capture(SessionId::new(1), &target, false)
        .expect("resume worker");
    host.deactivate(SessionId::new(1), &target, &[binding], &[])
        .expect("deactivate worker");
    sink.finish().expect("finish capture owner");

    let catalog = CaptureCatalog::read_current(&output).expect("capture checkpoint");
    assert!(catalog
        .entries()
        .iter()
        .any(|entry| entry.source() == "Window title"));
    assert!(catalog
        .entries()
        .iter()
        .any(|entry| entry.source() == "Handler removal tail"));
}

#[test]
fn iwh_005_timeout_immediately_reclaims_the_worker_process() {
    let root = tempdir().expect("temporary capture root");
    let output = root.path().join("isolated-worker-timeout.json");
    let sink = FileCaptureSink::start(
        CaptureConfiguration::new(
            CaptureSessionId::new("isolated-worker-timeout").expect("session id"),
            &output,
            100,
        )
        .expect("capture configuration"),
    )
    .expect("capture owner");
    let artifact = WorkerArtifact::open(worker_executable()).expect("worker artifact");
    let mut worker = ProcessIsolatedWorker::spawn(
        artifact,
        Duration::from_millis(100),
        &AdapterId::new("windows.uia.synthetic"),
        target_grant("target:hang-on-query"),
        producer(),
        11,
    )
    .expect("worker handshake");

    assert_eq!(
        worker.drain_once(&sink.ingress()),
        Err(WorkerHostError::Timeout)
    );
    assert_eq!(worker.health(), Err(WorkerHostError::Crashed));
    sink.finish().expect("finish capture owner");
}

#[test]
fn iwh_006_supervisor_restarts_a_timed_out_worker_with_a_new_generation() {
    let root = tempdir().expect("temporary capture root");
    let output = root.path().join("isolated-worker-restart.json");
    let sink = FileCaptureSink::start(
        CaptureConfiguration::new(
            CaptureSessionId::new("isolated-worker-restart").expect("session id"),
            &output,
            100,
        )
        .expect("capture configuration"),
    )
    .expect("capture owner");
    let artifact_id = PackageArtifactId::new("workers/uia-restart");
    let artifacts = WorkerArtifactCatalog::new([(artifact_id.clone(), worker_executable())])
        .expect("worker catalog");
    let mut host = IsolatedWorkerHost::new(artifacts, sink.ingress(), Duration::from_millis(100));
    let target = TargetInstance::new(
        TargetInstanceId::new("target-restart"),
        TargetFacts::new("windows", "x86_64"),
    );
    host.register_target(target.id().clone(), target_grant("target:hang-once"));
    let adapter_id = AdapterId::new("windows.uia.synthetic");
    let version = AdapterVersion::new(1, 0, 0);
    let binding = AdapterBinding {
        descriptor: AdapterDescriptor::new(
            adapter_id.clone(),
            version,
            ApplyModel::ObserveOnly,
            Placement::IsolatedWorker,
            [Feature::TextObserve],
        ),
        adapter_id,
        version,
        apply_model: ApplyModel::ObserveOnly,
        artifact_hash: ArtifactHash::sha256([9; 32]),
        host: AdapterHostBinding::IsolatedWorker {
            executable: artifact_id,
        },
        features: vec![Feature::TextObserve],
    };

    host.activate(&target, std::slice::from_ref(&binding))
        .expect("activate restart fixture");
    std::thread::sleep(Duration::from_millis(900));
    host.health(SessionId::new(1), &target, std::slice::from_ref(&binding))
        .expect("restarted worker health");
    host.deactivate(SessionId::new(1), &target, &[binding], &[])
        .expect("deactivate restarted worker");
    sink.finish().expect("finish capture owner");

    let catalog = CaptureCatalog::read_current(&output).expect("capture checkpoint");
    assert!(catalog
        .entries()
        .iter()
        .any(|entry| entry.source() == "Window title"));
}

#[test]
fn iwh_007_restart_budget_exhaustion_remains_visible_in_health() {
    let root = tempdir().expect("temporary capture root");
    let output = root.path().join("isolated-worker-restart-budget.json");
    let sink = FileCaptureSink::start(
        CaptureConfiguration::new(
            CaptureSessionId::new("isolated-worker-restart-budget").expect("session id"),
            &output,
            100,
        )
        .expect("capture configuration"),
    )
    .expect("capture owner");
    let artifact_id = PackageArtifactId::new("workers/uia-restart-budget");
    let artifacts = WorkerArtifactCatalog::new([(artifact_id.clone(), worker_executable())])
        .expect("worker catalog");
    let mut host = IsolatedWorkerHost::new(artifacts, sink.ingress(), Duration::from_millis(100));
    let target = TargetInstance::new(
        TargetInstanceId::new("target-restart-budget"),
        TargetFacts::new("windows", "x86_64"),
    );
    host.register_target(target.id().clone(), target_grant("target:hang-on-query"));
    let adapter_id = AdapterId::new("windows.uia.synthetic");
    let version = AdapterVersion::new(1, 0, 0);
    let binding = AdapterBinding {
        descriptor: AdapterDescriptor::new(
            adapter_id.clone(),
            version,
            ApplyModel::ObserveOnly,
            Placement::IsolatedWorker,
            [Feature::TextObserve],
        ),
        adapter_id,
        version,
        apply_model: ApplyModel::ObserveOnly,
        artifact_hash: ArtifactHash::sha256([10; 32]),
        host: AdapterHostBinding::IsolatedWorker {
            executable: artifact_id,
        },
        features: vec![Feature::TextObserve],
    };

    host.activate(&target, std::slice::from_ref(&binding))
        .expect("activate permanently stalled fixture");
    std::thread::sleep(Duration::from_millis(2_200));
    let health = host
        .health(SessionId::new(1), &target, std::slice::from_ref(&binding))
        .expect("terminal supervisor health");
    assert_eq!(health.failed_adapters().len(), 1);
    assert_eq!(health.diagnostics().len(), 1);
    assert_eq!(
        health.diagnostics()[0].code(),
        "isolated_worker_restart_exhausted"
    );
    let _ = host.deactivate(SessionId::new(1), &target, &[binding], &[]);
    sink.finish().expect("finish capture owner");
}

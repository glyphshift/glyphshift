use super::*;
use crate::acquisition::{map_acquisition_host_error, source_policy_for, AcquisitionExecutor};
use glyphshift_acquisition::{
    AcquisitionAdapter, AcquisitionCandidate, AcquisitionError, DesktopRect, Granularity,
    InteractiveTextAcquisition, Provenance,
};
use glyphshift_protocol::{ControllerRejection, ControllerWorkerTargetGrant};

#[derive(Clone, Copy)]
enum GrantBehavior {
    Granted,
    Failed(TransportFailure),
}

struct AcquisitionController {
    authorizations: Arc<AtomicUsize>,
    behavior: GrantBehavior,
}

impl ControllerTransport for AcquisitionController {
    fn handshake(
        &mut self,
        expected_extension: &ExtensionId,
        version: ProtocolVersion,
        nonce: ControllerNonce,
    ) -> Result<ControllerHello, TransportFailure> {
        Ok(ControllerHello::new(
            expected_extension.clone(),
            version,
            nonce,
        ))
    }

    fn inventory(&mut self) -> Result<ControllerInventory, TransportFailure> {
        Ok(ControllerInventory::new(
            [],
            [ControllerTarget::new(
                ControllerTargetToken::new("private-process-token"),
                "合成前台窗口",
                TargetFacts::new("windows", "x86_64"),
            )],
        ))
    }

    fn authorize_worker_target(
        &mut self,
        _target: &ControllerTargetToken,
    ) -> Result<ControllerWorkerTargetGrant, TransportFailure> {
        let sequence = self.authorizations.fetch_add(1, Ordering::SeqCst) + 1;
        match self.behavior {
            GrantBehavior::Granted => Ok(ControllerWorkerTargetGrant::new(
                "synthetic-process-v1",
                format!("ephemeral-grant-{sequence}"),
            )),
            GrantBehavior::Failed(error) => Err(error),
        }
    }

    fn terminate(&mut self) {}
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct AcquisitionCall {
    adapter_id: Box<str>,
    target: Box<str>,
    grant_platform: Box<str>,
    grant_payload: Box<str>,
    selection: InteractiveSelection,
    source_policy: SourcePolicy,
}

struct RecordingExecutor {
    calls: Arc<Mutex<Vec<AcquisitionCall>>>,
}

impl AcquisitionExecutor for RecordingExecutor {
    fn acquire(
        &self,
        adapter_id: &str,
        target: AuthorizedTarget,
        grant: ControllerWorkerTargetGrant,
        selection: InteractiveSelection,
        _cancellation: &DesktopAcquisitionCancellation,
    ) -> Result<AcquisitionResult, DesktopAcquisitionError> {
        let source_policy = source_policy_for(selection);
        self.calls
            .lock()
            .expect("acquisition calls")
            .push(AcquisitionCall {
                adapter_id: adapter_id.into(),
                target: target.as_str().into(),
                grant_platform: grant.platform().into(),
                grant_payload: grant.payload().into(),
                selection,
                source_policy,
            });
        let request = AcquisitionRequest::new(target, selection, source_policy);
        let point = match selection {
            InteractiveSelection::Point(point) => point,
            InteractiveSelection::Region(region) => DesktopPoint::new(region.left(), region.top()),
            InteractiveSelection::TextRange { start, .. } => start,
        };
        let provenance = match source_policy {
            SourcePolicy::VisualOnly => Provenance::Visual,
            SourcePolicy::Automatic | SourcePolicy::StructuredOnly => Provenance::Structured,
        };
        InteractiveTextAcquisition::new([
            Box::new(PointAdapter(point, provenance)) as Box<dyn AcquisitionAdapter>
        ])
        .acquire(&request)
        .map_err(|_| DesktopAcquisitionError::ProviderUnavailable)
    }
}

struct PointAdapter(DesktopPoint, Provenance);

impl AcquisitionAdapter for PointAdapter {
    fn provenance(&self) -> Provenance {
        self.1
    }

    fn acquire(
        &mut self,
        _request: &AcquisitionRequest,
    ) -> Result<Vec<AcquisitionCandidate>, AcquisitionError> {
        let anchor = DesktopRect::new(
            self.0.x(),
            self.0.y(),
            self.0.x().saturating_add(1),
            self.0.y().saturating_add(1),
        )
        .map_err(|_| AcquisitionError::ProviderUnavailable)?;
        Ok(vec![AcquisitionCandidate::new(
            "synthetic acquired text",
            [anchor],
            Granularity::Word,
        )])
    }
}

fn connect_acquisition_runtime(
    behavior: GrantBehavior,
    authorizations: Arc<AtomicUsize>,
    acquisition: Box<dyn AcquisitionExecutor>,
) -> DesktopRuntime<AcquisitionController> {
    let root = tempdir().expect("desktop acquisition root");
    let executable = root.path().join("SyntheticTarget.exe");
    fs::write(&executable, b"synthetic executable").expect("selected executable");
    let mut backend = open_test_backend(root.path().join("data"));
    let snapshot = backend
        .add_software(ExecutableSelection::new(executable))
        .expect("registered executable");
    let application_id = snapshot.software()[0].id();
    let spec = backend
        .runtime_spec(application_id)
        .expect("generic runtime spec");
    let runtime_library = root.path().join("runtime.dll");
    fs::write(&runtime_library, b"synthetic runtime").expect("runtime artifact");
    let artifacts = TargetArtifactCatalog::new(RuntimeArtifact::new(&runtime_library, [0; 32]), [])
        .expect("artifact catalog");
    let mut ledger = NonceLedger::new();
    DesktopRuntime::connect(
        AcquisitionController {
            authorizations,
            behavior,
        },
        application_id.into(),
        Vec::new(),
        spec.publication().clone(),
        AdapterRegistry::new(AdapterTrustPolicy::new([], [])),
        artifacts,
        WorkerArtifactCatalog::default(),
        acquisition,
        BTreeSet::new(),
        ProtocolVersion::new(1, 0),
        ControllerNonce::new([9; 32]),
        &mut ledger,
    )
    .expect("desktop acquisition runtime")
}

#[test]
fn desktop_acquisition_authorizes_each_point_request_and_returns_only_bounded_results() {
    let authorizations = Arc::new(AtomicUsize::new(0));
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut runtime = connect_acquisition_runtime(
        GrantBehavior::Granted,
        authorizations.clone(),
        Box::new(RecordingExecutor {
            calls: calls.clone(),
        }),
    );
    let cancellation = DesktopAcquisitionCancellation::new();

    for point in [DesktopPoint::new(-20, 30), DesktopPoint::new(40, 50)] {
        let result = runtime
            .acquire_point(1, "windows.uia.acquire", point, &cancellation)
            .expect("desktop point acquisition");
        assert_eq!(result.blocks()[0].source(), "synthetic acquired text");
    }

    assert_eq!(authorizations.load(Ordering::SeqCst), 2);
    let calls = calls.lock().expect("acquisition calls");
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].adapter_id.as_ref(), "windows.uia.acquire");
    assert_eq!(calls[0].target.as_ref(), "target-1");
    assert_eq!(calls[0].grant_platform.as_ref(), "synthetic-process-v1");
    assert_eq!(calls[0].grant_payload.as_ref(), "ephemeral-grant-1");
    assert_eq!(calls[1].grant_payload.as_ref(), "ephemeral-grant-2");
    assert_eq!(
        calls[0].selection,
        InteractiveSelection::Point(DesktopPoint::new(-20, 30))
    );
    assert_eq!(calls[0].source_policy, SourcePolicy::StructuredOnly);
}

#[test]
fn desktop_region_acquisition_uses_visual_only_with_a_fresh_grant() {
    let authorizations = Arc::new(AtomicUsize::new(0));
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut runtime = connect_acquisition_runtime(
        GrantBehavior::Granted,
        authorizations.clone(),
        Box::new(RecordingExecutor {
            calls: calls.clone(),
        }),
    );
    let region = DesktopRect::new(-320, -120, 320, 120).expect("bounded OCR region");

    runtime
        .acquire_region(
            1,
            "windows.ocr.acquire",
            region,
            &DesktopAcquisitionCancellation::new(),
        )
        .expect("desktop region acquisition");

    assert_eq!(authorizations.load(Ordering::SeqCst), 1);
    let calls = calls.lock().expect("acquisition calls");
    assert_eq!(calls[0].adapter_id.as_ref(), "windows.ocr.acquire");
    assert_eq!(calls[0].selection, InteractiveSelection::Region(region));
    assert_eq!(calls[0].source_policy, SourcePolicy::VisualOnly);
}

#[test]
fn desktop_acquisition_rejects_unknown_cancelled_and_unavailable_requests_before_worker_use() {
    let authorizations = Arc::new(AtomicUsize::new(0));
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut runtime = connect_acquisition_runtime(
        GrantBehavior::Granted,
        authorizations.clone(),
        Box::new(RecordingExecutor {
            calls: calls.clone(),
        }),
    );
    let cancellation = DesktopAcquisitionCancellation::new();
    assert_eq!(
        runtime.acquire_point(
            99,
            "windows.uia.acquire",
            DesktopPoint::new(0, 0),
            &cancellation,
        ),
        Err(DesktopAcquisitionError::UnknownTarget)
    );
    cancellation.cancel();
    assert_eq!(
        runtime.acquire_point(
            1,
            "windows.uia.acquire",
            DesktopPoint::new(0, 0),
            &cancellation,
        ),
        Err(DesktopAcquisitionError::Cancelled)
    );
    assert_eq!(authorizations.load(Ordering::SeqCst), 0);
    assert!(calls.lock().expect("acquisition calls").is_empty());

    let mut unknown_adapter_runtime = connect_acquisition_runtime(
        GrantBehavior::Granted,
        Arc::new(AtomicUsize::new(0)),
        Box::new(AcquisitionWorkerCatalog::default()),
    );
    assert_eq!(
        unknown_adapter_runtime.acquire_point(
            1,
            "unknown.acquire",
            DesktopPoint::new(0, 0),
            &DesktopAcquisitionCancellation::new(),
        ),
        Err(DesktopAcquisitionError::WorkerUnavailable)
    );
}

#[test]
fn desktop_acquisition_maps_controller_and_worker_failures_to_stable_product_errors() {
    for (failure, expected) in [
        (TransportFailure::Timeout, DesktopAcquisitionError::TimedOut),
        (
            TransportFailure::Rejected(ControllerRejection::Unknown),
            DesktopAcquisitionError::TargetUnavailable,
        ),
    ] {
        let mut runtime = connect_acquisition_runtime(
            GrantBehavior::Failed(failure),
            Arc::new(AtomicUsize::new(0)),
            Box::new(AcquisitionWorkerCatalog::default()),
        );
        assert_eq!(
            runtime.acquire_point(
                1,
                "windows.uia.acquire",
                DesktopPoint::new(1, 2),
                &DesktopAcquisitionCancellation::new(),
            ),
            Err(expected)
        );
    }

    for (failure, expected) in [
        (
            AcquisitionWorkerHostError::AcquisitionRejected(AcquisitionError::PermissionDenied),
            DesktopAcquisitionError::PermissionDenied,
        ),
        (
            AcquisitionWorkerHostError::AcquisitionRejected(AcquisitionError::NoText),
            DesktopAcquisitionError::NoText,
        ),
        (
            AcquisitionWorkerHostError::AcquisitionRejected(AcquisitionError::ProviderUnavailable),
            DesktopAcquisitionError::ProviderUnavailable,
        ),
        (
            AcquisitionWorkerHostError::AcquisitionRejected(AcquisitionError::TargetMismatch),
            DesktopAcquisitionError::TargetUnavailable,
        ),
        (
            AcquisitionWorkerHostError::Timeout,
            DesktopAcquisitionError::TimedOut,
        ),
        (
            AcquisitionWorkerHostError::Cancelled,
            DesktopAcquisitionError::Cancelled,
        ),
        (
            AcquisitionWorkerHostError::Crashed,
            DesktopAcquisitionError::WorkerUnavailable,
        ),
        (
            AcquisitionWorkerHostError::MalformedMessage,
            DesktopAcquisitionError::WorkerUnavailable,
        ),
    ] {
        assert_eq!(map_acquisition_host_error(failure), expected);
    }
}

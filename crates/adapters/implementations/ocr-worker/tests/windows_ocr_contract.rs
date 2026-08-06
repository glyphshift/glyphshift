#![cfg(windows)]

use glyphshift_acquisition::{
    AcquisitionError, AcquisitionRequest, AuthorizedTarget, DesktopPoint, DesktopRect,
    InteractiveSelection, Provenance, SourcePolicy,
};
use glyphshift_acquisition_worker_host::{
    AcquisitionWorkerArtifact, AcquisitionWorkerBinding, AcquisitionWorkerHost,
    AcquisitionWorkerHostError, CancellationToken,
};
use glyphshift_adapter_ocr_worker::ACQUISITION_ADAPTER_ID;
use glyphshift_worker_process_grant::{process_started_at, WINDOWS_PROCESS_GRANT_PLATFORM};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use windows_capture::window::Window;

#[test]
fn worker_rejects_non_region_and_non_visual_requests_before_capture() {
    let (host, binding, target) = current_process_host_and_binding(ACQUISITION_ADAPTER_ID);
    let point = AcquisitionRequest::new(
        target.clone(),
        InteractiveSelection::Point(DesktopPoint::new(0, 0)),
        SourcePolicy::VisualOnly,
    );
    let structured = AcquisitionRequest::new(
        target,
        InteractiveSelection::Region(DesktopRect::new(0, 0, 1, 1).expect("synthetic region")),
        SourcePolicy::StructuredOnly,
    );

    for request in [&point, &structured] {
        assert_eq!(
            host.acquire(&binding, request, &CancellationToken::new())
                .err(),
            Some(AcquisitionWorkerHostError::AcquisitionRejected(
                AcquisitionError::ProviderUnavailable
            ))
        );
    }
}

#[test]
fn worker_rejects_a_grant_bound_to_another_adapter() {
    let (host, binding, target) = current_process_host_and_binding("windows.other.acquire");
    let request = AcquisitionRequest::new(
        target,
        InteractiveSelection::Region(DesktopRect::new(0, 0, 1, 1).expect("synthetic region")),
        SourcePolicy::VisualOnly,
    );

    assert_eq!(
        host.acquire(&binding, &request, &CancellationToken::new())
            .err(),
        Some(AcquisitionWorkerHostError::AcquisitionRejected(
            AcquisitionError::TargetMismatch
        ))
    );
}

#[test]
#[ignore = "requires an authorized target and staged local verified OCR artifacts"]
fn supervised_worker_runs_wgc_and_tesseract_through_the_one_shot_protocol() {
    let process_id = std::env::var("GLYPHSHIFT_OCR_TARGET_PID")
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .filter(|value| *value > 0)
        .expect("authorized OCR target process");
    let started_at = process_started_at(process_id).expect("target start time");
    let window = Window::enumerate()
        .expect("enumerate windows")
        .into_iter()
        .find(|window| window.process_id().ok() == Some(process_id))
        .expect("authorized top-level window");
    let rect = window.rect().expect("authorized window rect");
    let selection = DesktopRect::new(rect.left, rect.top, rect.right, rect.bottom)
        .expect("authorized window selection");
    let artifact_root = PathBuf::from(
        std::env::var_os("GLYPHSHIFT_OCR_ARTIFACT_ROOT").expect("staged OCR artifact root"),
    );
    let artifact = AcquisitionWorkerArtifact::open(artifact_root.join(format!(
        "glyphshift-adapter-ocr-acquisition-worker{}",
        std::env::consts::EXE_SUFFIX
    )))
    .expect("OCR worker artifact");
    let host = AcquisitionWorkerHost::new(artifact, Duration::from_secs(8)).expect("OCR host");
    let target = AuthorizedTarget::new("authorized-ocr-target").expect("target identity");
    let binding = AcquisitionWorkerBinding::new(
        target.clone(),
        ACQUISITION_ADAPTER_ID,
        WINDOWS_PROCESS_GRANT_PLATFORM,
        format!("{process_id}:{started_at}"),
    )
    .expect("OCR binding");
    let request = AcquisitionRequest::new(
        target,
        InteractiveSelection::Region(selection),
        SourcePolicy::VisualOnly,
    );

    let started = Instant::now();
    let result = host
        .acquire(&binding, &request, &CancellationToken::new())
        .expect("supervised OCR acquisition");

    assert!(!result.blocks().is_empty());
    assert!(result.blocks().len() <= 256);
    assert!(result.blocks().iter().all(|block| {
        block.provenance() == Provenance::Visual
            && !block.source().trim().is_empty()
            && block
                .anchors()
                .iter()
                .all(|anchor| selection.intersection(*anchor) == Some(*anchor))
    }));
    eprintln!(
        "OCR_WORKER_BLOCKS={} OCR_WORKER_MILLIS={}",
        result.blocks().len(),
        started.elapsed().as_millis()
    );
}

fn current_process_host_and_binding(
    adapter_id: &str,
) -> (
    AcquisitionWorkerHost,
    AcquisitionWorkerBinding,
    AuthorizedTarget,
) {
    let artifact = AcquisitionWorkerArtifact::open(PathBuf::from(env!(
        "CARGO_BIN_EXE_glyphshift-adapter-ocr-acquisition-worker"
    )))
    .expect("OCR worker artifact");
    let host = AcquisitionWorkerHost::new(artifact, Duration::from_secs(3)).expect("OCR host");
    let process_id = std::process::id();
    let started_at = process_started_at(process_id).expect("current process start time");
    let target = AuthorizedTarget::new("synthetic-ocr-target").expect("target identity");
    let binding = AcquisitionWorkerBinding::new(
        target.clone(),
        adapter_id,
        WINDOWS_PROCESS_GRANT_PLATFORM,
        format!("{process_id}:{started_at}"),
    )
    .expect("OCR binding");
    (host, binding, target)
}

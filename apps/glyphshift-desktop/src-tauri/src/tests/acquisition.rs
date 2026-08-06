use super::*;
use crate::acquisition::DesktopPointAcquisitionRequest;

#[test]
fn application_compiles_the_runtime_spec_and_returns_a_platform_neutral_result() {
    let (mut application, calls, software_id, _root) = workflow_application();
    let request = DesktopPointAcquisitionRequest::new(
        software_id.clone(),
        7,
        "windows.uia.acquire",
        -20,
        30,
        "request-1",
    );

    let result = application
        .acquire_point(&request, &DesktopAcquisitionCancellation::new())
        .expect("point acquisition product result");

    assert_eq!(
        calls.lock().expect("runtime call log").point_acquisitions,
        vec![(
            software_id,
            7,
            Box::<str>::from("windows.uia.acquire"),
            DesktopPoint::new(-20, 30),
        )]
    );
    let value = serde_json::to_value(result).expect("serialize acquisition result");
    assert_eq!(value["blocks"][0]["source"], "Open");
    let encoded = value.to_string().to_ascii_lowercase();
    for forbidden in ["pid", "grant", "token", "path", "worker", "executable"] {
        assert!(!encoded.contains(forbidden));
    }
}

#[test]
fn application_maps_unknown_software_and_pre_cancelled_requests_to_stable_codes() {
    let (mut application, _calls, _software_id, _root) = workflow_application();
    let unknown = DesktopPointAcquisitionRequest::new(
        "software.unknown",
        1,
        "windows.uia.acquire",
        10,
        20,
        "request-unknown",
    );
    assert_eq!(
        application.acquire_point(&unknown, &DesktopAcquisitionCancellation::new()),
        Err(CommandError::new("acquisition.software_not_found"))
    );

    let known_id = application.backend.snapshot().software()[0].id().to_owned();
    let cancelled = DesktopPointAcquisitionRequest::new(
        known_id,
        1,
        "windows.uia.acquire",
        10,
        20,
        "request-cancelled",
    );
    let cancellation = DesktopAcquisitionCancellation::new();
    cancellation.cancel();
    assert_eq!(
        application.acquire_point(&cancelled, &cancellation),
        Err(CommandError::new("acquisition.cancelled"))
    );
}

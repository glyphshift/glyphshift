use super::*;
use crate::hybrid::resolve_runtime_activation;
use crate::isolated::map_worker_error;
use crate::worker::{valid_worker_code, MAX_HEALTH_CODE_BYTES};
use glyphshift_domain::AdapterId;
use glyphshift_session::{BoundFeature, HostActivation, HostFailure, HostOperationFailure};

#[test]
fn mixed_activation_keeps_isolated_features_when_target_process_fails() {
    let target_feature = BoundFeature::new(
        AdapterId::new("windows.gdi.text-out"),
        glyphshift_adapter_registry::AdapterVersion::new(1, 0, 0),
        glyphshift_domain::Feature::TextReplace,
    );
    let isolated_feature = BoundFeature::new(
        AdapterId::new("windows.uia.observe"),
        glyphshift_adapter_registry::AdapterVersion::new(1, 0, 0),
        glyphshift_domain::Feature::TextObserve,
    );

    let (active, activation) = resolve_runtime_activation(
        Some(Err(HostFailure::HandshakeRejected)),
        Some(Ok(())),
        vec![target_feature.clone()],
        vec![isolated_feature.clone()],
    )
    .expect("the healthy isolated placement should keep the session alive");

    assert!(!active.target_process);
    assert!(active.isolated_worker);
    assert_eq!(
        activation,
        HostActivation::reported([isolated_feature], [target_feature])
    );
}

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

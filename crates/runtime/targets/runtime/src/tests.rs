use super::*;
use glyphshift_capture::{CaptureProducerConfiguration, CaptureProducerId};
use glyphshift_domain::{Generation, RouteProgram};
use glyphshift_translation::{FontPolicy, TranslationSnapshot};

#[test]
fn target_runtime_drains_observations_only_in_batch_producer_mode() {
    let publication = RuntimePublication::new(
        RouteProgram::direct("capture"),
        TranslationSnapshot::empty(Generation::new(1)),
        FontPolicy::empty(),
    );
    let deployment = TargetRuntimeDeployment::new(publication, std::iter::empty())
        .with_observation_producer(
            CaptureProducerConfiguration::new(
                CaptureProducerId::new("target-1").expect("producer id"),
                2,
            )
            .expect("producer configuration"),
        );
    activate_deployment(deployment).expect("activate batch producer");

    let context = Box::into_raw(Box::new(NativeDecisionContext {
        adapter_id: "synthetic.observe".into(),
    }));
    let observe = |source: &str| {
        let source = source.encode_utf16().collect::<Vec<_>>();
        decide_utf16(
            context.cast(),
            source.as_ptr(),
            source.len() as u32,
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            0,
        )
    };
    assert_eq!(observe("Open").status, STATUS_OK);
    let first = query_observations().expect("first observation batch");
    assert_eq!(first.producer_id().as_str(), "target-1");
    assert_eq!(first.generation(), 2);
    assert_eq!(first.records().len(), 1);
    assert_eq!(first.records()[0].source(), "Open");

    control_capture(CaptureRuntimeControl::new(true)).expect("pause producer");
    assert_eq!(observe("Ignored").status, STATUS_OK);
    assert!(query_observations()
        .expect("paused observation batch")
        .records()
        .is_empty());
    control_capture(CaptureRuntimeControl::new(false)).expect("resume producer");
    deactivate_runtime().expect("deactivate batch producer");
    assert_eq!(
        query_observations(),
        Err(TargetRuntimeError::RuntimeUnavailable)
    );
    unsafe {
        drop(Box::from_raw(context));
    }
}

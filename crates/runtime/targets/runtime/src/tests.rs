use super::*;
use glyphshift_capture::{CaptureProducerConfiguration, CaptureProducerId};
use glyphshift_domain::{Generation, RouteProgram};
use glyphshift_translation::{FontPolicy, TranslationSnapshot};
static TEST_RUNTIME: Mutex<()> = Mutex::new(());

#[test]
fn target_runtime_drains_observations_only_in_batch_producer_mode() {
    let _serial = TEST_RUNTIME.lock().unwrap();
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
        source_policy: SourceTextPolicy::Exact,
        text_runs: Mutex::new(glyphshift_domain::TextRunResolver::default()),
        text_host: OnceLock::new(),
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

#[test]
fn native_runtime_matches_capture_key_for_edge_padded_text_and_preserves_padding() {
    let _serial = TEST_RUNTIME.lock().unwrap();
    assert_eq!(SourceTextPolicy::Exact.key(" Add "), "Add");
    let publication = RuntimePublication::new(
        RouteProgram::direct("capture"),
        TranslationSnapshot::empty(Generation::new(1))
            .with_entry("capture", "Add", "添加")
            .with_entry("capture", " Exact ", "精确优先")
            .with_entry("capture", "Exact", "去空格回退"),
        FontPolicy::empty(),
    );
    activate_deployment(TargetRuntimeDeployment::new(
        publication,
        std::iter::empty(),
    ))
    .unwrap();
    let context = Box::into_raw(Box::new(NativeDecisionContext {
        adapter_id: "synthetic.padded".into(),
        source_policy: SourceTextPolicy::Exact,
        text_runs: Mutex::new(glyphshift_domain::TextRunResolver::default()),
        text_host: OnceLock::new(),
    }));
    let decide = |source: &str| {
        let source = source.encode_utf16().collect::<Vec<_>>();
        let mut output = [0_u16; 64];
        let decision = decide_utf16(
            context.cast(),
            source.as_ptr(),
            source.len() as u32,
            output.as_mut_ptr(),
            output.len() as u32,
            std::ptr::null_mut(),
            0,
        );
        let text = String::from_utf16(&output[..decision.text_len as usize]).unwrap();
        (decision, text)
    };

    let padded = decide(" Add ");
    assert_ne!(padded.0.decision_bits & DECISION_TEXT_REPLACE, 0);
    assert_eq!(padded.1, " 添加 ");

    let exact = decide(" Exact ");
    assert_ne!(exact.0.decision_bits & DECISION_TEXT_REPLACE, 0);
    assert_eq!(exact.1, "精确优先");

    deactivate_runtime().unwrap();
    unsafe {
        drop(Box::from_raw(context));
    }
}

#[test]
fn structured_text_resolves_before_capture_and_never_replaces_past_fragments() {
    use glyphshift_adapter_native_abi::*;
    let _serial = TEST_RUNTIME.lock().unwrap();
    let publication = RuntimePublication::new(
        RouteProgram::direct("capture"),
        TranslationSnapshot::empty(Generation::new(1))
            .with_entry("capture", "Open", "打开")
            .with_entry("capture", "是", "Yes"),
        FontPolicy::empty(),
    );
    let deployment = TargetRuntimeDeployment::new(publication, std::iter::empty())
        .with_observation_producer(
            CaptureProducerConfiguration::new(CaptureProducerId::new("text-run-test").unwrap(), 1)
                .unwrap(),
        );
    activate_deployment(deployment).unwrap();
    let host = native_host("synthetic.structured", SourceTextPolicy::Exact);
    runtime_state()
        .lock()
        .unwrap()
        .as_mut()
        .unwrap()
        .native_hosts
        .push(host);
    let host = unsafe { &*(host as *const NativeRuntimeHostV1) };
    let send = |kind, ordinal, source: &str| {
        let source = source.encode_utf16().collect::<Vec<_>>();
        let event = NativeTextEventV1 {
            kind,
            surface: 1,
            run: 1,
            epoch: 1,
            ordinal,
            ..NativeTextEventV1::complete_draw(&source)
        };
        let mut text = [0; 64];
        let mut font = [0; 64];
        let decision = text_host::decide_text(
            host.context,
            &event,
            text.as_mut_ptr(),
            64,
            font.as_mut_ptr(),
            64,
        );
        (
            decision,
            String::from_utf16(&text[..decision.text_len as usize]).unwrap(),
        )
    };
    assert_eq!(send(TEXT_EVENT_GLYPH_RASTER, 0, "").0.decision_bits, 0);
    assert!(query_observations().unwrap().records().is_empty());
    assert_eq!(send(TEXT_EVENT_FRAGMENT, 0, "Op").0.status, STATUS_OK);
    assert_eq!(send(TEXT_EVENT_FRAGMENT, 1, "en").0.status, STATUS_OK);
    assert!(query_observations().unwrap().records().is_empty());
    let assembled = send(TEXT_EVENT_FINISH, 2, "");
    assert_eq!(
        assembled.0.decision_bits, 0,
        "assembled capture cannot replace already drawn calls"
    );
    assert_eq!(query_observations().unwrap().records()[0].source(), "Open");
    assert_eq!(send(TEXT_EVENT_OBSERVE, 0, "Open").0.decision_bits, 0);
    assert_eq!(send(TEXT_EVENT_DRAW, 0, "Open").1, "打开");
    assert_eq!(send(TEXT_EVENT_RETAINED, 0, "是").1, "Yes");
    assert!(!query_observations().unwrap().records().is_empty());
    let child = native_host("synthetic.raster", SourceTextPolicy::Exact);
    runtime_state()
        .lock()
        .unwrap()
        .as_mut()
        .unwrap()
        .native_hosts
        .push(child);
    let child = unsafe { &*(child as *const NativeRuntimeHostV1) };
    let raster = || {
        let mut output = [0; 16];
        (child.decide_utf16)(
            child.context,
            [0x662f].as_ptr(),
            1,
            output.as_mut_ptr(),
            16,
            std::ptr::null_mut(),
            0,
        )
    };
    // An untranslated outer character must not hide a complete inner string.
    let untouched = text_host::enter_scope(host.context);
    assert_eq!(send(TEXT_EVENT_DRAW, 0, "O").0.decision_bits, 0);
    let too_small = "Open".encode_utf16().collect::<Vec<_>>();
    assert_ne!(
        (host.decide_utf16)(
            host.context,
            too_small.as_ptr(),
            too_small.len() as u32,
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            0
        )
        .status,
        STATUS_OK
    );
    let inner = text_host::enter_scope(child.context);
    let source = "Open".encode_utf16().collect::<Vec<_>>();
    let mut output = [0; 64];
    let event = NativeTextEventV1::complete_draw(&source);
    let decision = text_host::decide_text(
        child.context,
        &event,
        output.as_mut_ptr(),
        64,
        std::ptr::null_mut(),
        0,
    );
    assert_eq!(
        String::from_utf16(&output[..decision.text_len as usize]).unwrap(),
        "打开"
    );
    // The inner replacement protects its descendants, even with an untouched outer scope.
    assert_eq!(send(TEXT_EVENT_DRAW, 0, "是").0.decision_bits, 0);
    text_host::leave_scope(child.context, inner);
    assert_ne!(
        raster().decision_bits & DECISION_TEXT_REPLACE,
        0,
        "legacy callbacks also pass through an untranslated parent"
    );
    text_host::leave_scope(host.context, untouched);
    let observed = query_observations().unwrap();
    assert!(observed
        .records()
        .iter()
        .any(|record| record.source() == "Open"));
    let token = text_host::enter_scope(host.context);
    assert_ne!(token, 0);
    assert_eq!(send(TEXT_EVENT_DRAW, 0, "Open").1, "打开");
    assert_eq!(
        raster().decision_bits,
        0,
        "subordinate glyph drawing must not be translated again"
    );
    let nested = text_host::enter_scope(child.context);
    assert_eq!(raster().decision_bits, 0);
    text_host::leave_scope(child.context, nested);
    // Another rendering thread is not inside the parent's draw call.
    let child_context = child.context as usize;
    std::thread::spawn(move || {
        let text = "Outside".encode_utf16().collect::<Vec<_>>();
        decide_utf16(
            child_context as *mut _,
            text.as_ptr(),
            text.len() as u32,
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            0,
        )
    })
    .join()
    .unwrap();
    text_host::leave_scope(host.context, token);
    let records = query_observations().unwrap();
    assert!(records
        .records()
        .iter()
        .any(|record| record.source() == "Open"));
    assert!(records
        .records()
        .iter()
        .any(|record| record.source() == "Outside"));
    assert!(!records
        .records()
        .iter()
        .any(|record| record.source() == "是"));
    assert_ne!(
        raster().decision_bits & DECISION_TEXT_REPLACE,
        0,
        "independent single labels remain translatable"
    );
    query_observations().unwrap();
    // A fresh lifecycle cannot complete a partial run from before a pause.
    control_capture(CaptureRuntimeControl::new(true)).unwrap();
    control_capture(CaptureRuntimeControl::new(false)).unwrap();
    assert_eq!(send(TEXT_EVENT_FRAGMENT, 0, "Op").0.status, STATUS_OK);
    control_capture(CaptureRuntimeControl::new(true)).unwrap();
    control_capture(CaptureRuntimeControl::new(false)).unwrap();
    assert_ne!(send(TEXT_EVENT_FINISH, 1, "").0.status, STATUS_OK);
    assert!(query_observations().unwrap().records().is_empty());
    control_capture(CaptureRuntimeControl::new(false)).unwrap();
    assert_eq!(send(TEXT_EVENT_FRAGMENT, 0, "Prefix").0.status, STATUS_OK);
    let invalid = NativeTextEventV1 {
        source_len: u32::MAX,
        ..NativeTextEventV1::complete_draw(&[65])
    };
    assert_eq!(
        text_host::decide_text(
            host.context,
            &invalid,
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            0
        )
        .status,
        STATUS_INVALID_TEXT_EVENT
    );
    assert_ne!(send(TEXT_EVENT_FINISH, 1, "").0.status, STATUS_OK);
    deactivate_runtime().unwrap();
    assert_ne!(send(TEXT_EVENT_FRAGMENT, 0, "Late").0.status, STATUS_OK);
}

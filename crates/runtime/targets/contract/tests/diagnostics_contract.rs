use glyphshift_target_runtime_contract::{
    RuntimeDiagnosticsControl, RuntimeFontOutcome, RuntimeTextOutcome, RuntimeTraceBatch,
    RuntimeTraceRecord, RuntimeTraceStatus,
};

#[test]
fn trc_001_round_trips_a_bounded_privacy_scoped_trace_batch() {
    let batch = RuntimeTraceBatch::new(
        [RuntimeTraceRecord::new(
            "adapter.example",
            "Open",
            RuntimeTraceStatus::Matched,
            RuntimeTextOutcome::Replaced,
            RuntimeFontOutcome::Protected,
            7,
            [1; 32],
            [2; 32],
            [3; 32],
        )],
        4,
    );

    let encoded = batch.encode_json().expect("trace batch encode");
    assert!(!encoded.contains("route"));
    assert!(!encoded.contains("process"));
    assert!(!encoded.contains("window"));
    let decoded = RuntimeTraceBatch::decode_json(&encoded).expect("trace batch decode");

    assert_eq!(decoded, batch);
    assert_eq!(decoded.records()[0].source_text(), "Open");
    assert_eq!(decoded.dropped(), 4);
}

#[test]
fn trc_002_round_trips_explicit_diagnostics_control() {
    let control = RuntimeDiagnosticsControl::new(true);

    let encoded = control.encode_json().expect("diagnostics control encode");
    let decoded =
        RuntimeDiagnosticsControl::decode_json(&encoded).expect("diagnostics control decode");

    assert!(decoded.enabled());
}

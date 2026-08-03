use glyphshift_controller_sdk::{
    serve, ControllerPlugin, PluginError, Response, ResponseEnvelope, WireInventory, WireRecipe,
    WireRuntimeFontOutcome, WireRuntimeTextOutcome, WireRuntimeTraceBatch, WireRuntimeTraceRecord,
    WireRuntimeTraceStatus, PROTOCOL_SCHEMA,
};
use std::io::Cursor;

struct DiagnosticsPlugin;

impl ControllerPlugin for DiagnosticsPlugin {
    fn inventory(&mut self) -> Result<WireInventory, PluginError> {
        Ok(WireInventory::default())
    }

    fn launch(&mut self, _installation_token: &str) -> Result<(), PluginError> {
        Err(PluginError::new("unsupported"))
    }

    fn prepare(
        &mut self,
        _target_token: &str,
        _requested_features: &[glyphshift_controller_sdk::WireFeature],
    ) -> Result<WireRecipe, PluginError> {
        Err(PluginError::new("unsupported"))
    }

    fn control_diagnostics(
        &mut self,
        target_token: &str,
        enabled: bool,
    ) -> Result<(), PluginError> {
        assert_eq!(target_token, "target:1");
        assert!(enabled);
        Ok(())
    }

    fn query_diagnostics(
        &mut self,
        target_token: &str,
    ) -> Result<WireRuntimeTraceBatch, PluginError> {
        assert_eq!(target_token, "target:1");
        Ok(WireRuntimeTraceBatch {
            records: vec![WireRuntimeTraceRecord {
                adapter_id: "adapter.example".into(),
                source_text: "Open".into(),
                status: WireRuntimeTraceStatus::Matched,
                text: WireRuntimeTextOutcome::Replaced,
                font: WireRuntimeFontOutcome::Protected,
                generation: 9,
                publication_identity: [1; 32],
                translation_digest: [2; 32],
                font_policy_digest: [3; 32],
            }],
            dropped: 2,
        })
    }
}

#[test]
fn ctl_sdk_001_round_trips_runtime_diagnostics_commands() {
    let input = format!(
        "{{\"schema\":\"{PROTOCOL_SCHEMA}\",\"request_id\":1,\"kind\":\"control_diagnostics\",\"target_token\":\"target:1\",\"enabled\":true}}\n\
         {{\"schema\":\"{PROTOCOL_SCHEMA}\",\"request_id\":2,\"kind\":\"query_diagnostics\",\"target_token\":\"target:1\"}}\n\
         {{\"schema\":\"{PROTOCOL_SCHEMA}\",\"request_id\":3,\"kind\":\"terminate\"}}\n"
    );
    let mut output = Vec::new();

    serve(DiagnosticsPlugin, Cursor::new(input), &mut output).expect("serve diagnostics protocol");

    let responses = String::from_utf8(output)
        .expect("UTF-8 responses")
        .lines()
        .map(|line| serde_json::from_str::<ResponseEnvelope>(line).expect("response envelope"))
        .collect::<Vec<_>>();
    assert_eq!(responses.len(), 2);
    assert_eq!(
        responses[0].response,
        Response::DiagnosticsControlled { enabled: true }
    );
    let Response::RuntimeDiagnostics { batch } = &responses[1].response else {
        panic!("expected runtime diagnostics response");
    };
    assert_eq!(batch.records[0].source_text, "Open");
    assert_eq!(batch.dropped, 2);
}

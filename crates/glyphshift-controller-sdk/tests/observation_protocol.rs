use glyphshift_controller_sdk::{
    serve, ControllerPlugin, PluginError, Response, ResponseEnvelope, WireCaptureObservationBatch,
    WireCaptureObservationRecord, WireInventory, WireRecipe, PROTOCOL_SCHEMA,
};
use std::io::Cursor;

struct ObservationPlugin;

impl ControllerPlugin for ObservationPlugin {
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

    fn query_observations(
        &mut self,
        target_token: &str,
    ) -> Result<WireCaptureObservationBatch, PluginError> {
        assert_eq!(target_token, "target:1");
        Ok(WireCaptureObservationBatch {
            producer_id: "target-1".into(),
            generation: 9,
            dropped_total: 2,
            records: vec![WireCaptureObservationRecord {
                sequence: 4,
                adapter_id: "adapter.example".into(),
                source: "Open".into(),
            }],
        })
    }
}

#[test]
fn ctl_sdk_002_round_trips_runtime_observation_batches() {
    let input = format!(
        "{{\"schema\":\"{PROTOCOL_SCHEMA}\",\"request_id\":1,\"kind\":\"query_observations\",\"target_token\":\"target:1\"}}\n\
         {{\"schema\":\"{PROTOCOL_SCHEMA}\",\"request_id\":2,\"kind\":\"terminate\"}}\n"
    );
    let mut output = Vec::new();

    serve(ObservationPlugin, Cursor::new(input), &mut output).expect("serve observation protocol");

    let responses = String::from_utf8(output)
        .expect("UTF-8 responses")
        .lines()
        .map(|line| serde_json::from_str::<ResponseEnvelope>(line).expect("response envelope"))
        .collect::<Vec<_>>();
    assert_eq!(responses.len(), 1);
    let Response::RuntimeObservations { batch } = &responses[0].response else {
        panic!("expected runtime observations response");
    };
    assert_eq!(batch.producer_id, "target-1");
    assert_eq!(batch.generation, 9);
    assert_eq!(batch.dropped_total, 2);
    assert_eq!(batch.records[0].sequence, 4);
    assert_eq!(batch.records[0].source, "Open");
}

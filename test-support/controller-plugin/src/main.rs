use glyphshift_controller_sdk::{
    serve_stdio, ControllerPlugin, PluginError, WireAdapterRequirement,
    WireCaptureObservationBatch, WireCaptureObservationRecord, WireControllerConfiguration,
    WireControllerLossPolicy, WireFeature, WireInstallation, WireInventory, WireRecipe,
    WireRuntimeFontOutcome, WireRuntimeTextOutcome, WireRuntimeTraceBatch, WireRuntimeTraceRecord,
    WireRuntimeTraceStatus, WireTarget, WireWorkerTargetGrant,
};

#[derive(Default)]
struct SyntheticController {
    adapter_requirements: Vec<WireAdapterRequirement>,
}

impl ControllerPlugin for SyntheticController {
    fn configure(
        &mut self,
        _extension_id: &str,
        configuration: &WireControllerConfiguration,
    ) -> Result<(), PluginError> {
        if configuration.executable_names != ["SyntheticEditor.exe"]
            || configuration.descendant_executable_names != ["SyntheticRenderer.exe"]
        {
            return Err(PluginError::new("invalid_target_configuration"));
        }
        self.adapter_requirements = configuration.adapter_requirements.clone();
        Ok(())
    }

    fn inventory(&mut self) -> Result<WireInventory, PluginError> {
        Ok(WireInventory {
            installations: vec![WireInstallation {
                token: "installation:stable".into(),
                display_name: "Synthetic installation".into(),
            }],
            targets: vec![
                WireTarget {
                    token: "target:stable".into(),
                    display_name: "Synthetic target".into(),
                    operating_system: "windows".into(),
                    architecture: "x86_64".into(),
                },
                WireTarget {
                    token: "target:crash".into(),
                    display_name: "Synthetic crash target".into(),
                    operating_system: "windows".into(),
                    architecture: "x86_64".into(),
                },
                WireTarget {
                    token: "target:rejected".into(),
                    display_name: "Synthetic rejected target".into(),
                    operating_system: "windows".into(),
                    architecture: "x86_64".into(),
                },
            ],
        })
    }

    fn launch(&mut self, installation_token: &str) -> Result<(), PluginError> {
        if installation_token == "installation:stable" {
            Ok(())
        } else {
            Err(PluginError::new("installation_not_found"))
        }
    }

    fn prepare(
        &mut self,
        target_token: &str,
        requested_features: &[WireFeature],
    ) -> Result<WireRecipe, PluginError> {
        if target_token == "target:crash" {
            std::process::exit(23);
        }
        if target_token == "target:rejected" {
            return Err(PluginError::new(
                "runtime_activation_failed:runtime_module_unavailable",
            ));
        }
        if target_token != "target:stable" {
            return Err(PluginError::new("target_not_found"));
        }
        Ok(WireRecipe {
            adapters: self
                .adapter_requirements
                .iter()
                .cloned()
                .map(|mut requirement| {
                    requirement
                        .features
                        .retain(|feature| requested_features.contains(feature));
                    requirement
                })
                .collect(),
            controller_loss_policy: WireControllerLossPolicy::Degrade,
        })
    }

    fn control_diagnostics(
        &mut self,
        target_token: &str,
        _enabled: bool,
    ) -> Result<(), PluginError> {
        (target_token == "target:stable")
            .then_some(())
            .ok_or_else(|| PluginError::new("target_not_found"))
    }

    fn authorize_worker_target(
        &mut self,
        target_token: &str,
    ) -> Result<WireWorkerTargetGrant, PluginError> {
        if target_token != "target:stable" {
            return Err(PluginError::new("target_not_found"));
        }
        Ok(WireWorkerTargetGrant {
            platform: "synthetic-process-v1".into(),
            payload: "target:authorized".into(),
        })
    }

    fn query_diagnostics(
        &mut self,
        target_token: &str,
    ) -> Result<WireRuntimeTraceBatch, PluginError> {
        if target_token != "target:stable" {
            return Err(PluginError::new("target_not_found"));
        }
        Ok(WireRuntimeTraceBatch {
            records: vec![WireRuntimeTraceRecord {
                adapter_id: "example.synthetic.process-inline".into(),
                source_text: "Open".into(),
                status: WireRuntimeTraceStatus::Matched,
                text: WireRuntimeTextOutcome::Replaced,
                font: WireRuntimeFontOutcome::Protected,
                generation: 7,
                publication_identity: [1; 32],
                translation_digest: [2; 32],
                font_policy_digest: [3; 32],
            }],
            dropped: 2,
        })
    }

    fn query_observations(
        &mut self,
        target_token: &str,
    ) -> Result<WireCaptureObservationBatch, PluginError> {
        if target_token != "target:stable" {
            return Err(PluginError::new("target_not_found"));
        }
        Ok(WireCaptureObservationBatch {
            producer_id: "synthetic-target".into(),
            generation: 7,
            dropped_total: 3,
            records: vec![WireCaptureObservationRecord {
                sequence: 11,
                adapter_id: "example.synthetic.process-inline".into(),
                source: "Open".into(),
            }],
        })
    }
}

fn main() -> std::io::Result<()> {
    serve_stdio(SyntheticController::default())
}

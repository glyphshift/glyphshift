use glyphshift_controller_sdk::{
    serve_stdio, ControllerPlugin, PluginError, WireAdapterRequirement,
    WireControllerConfiguration, WireControllerLossPolicy, WireFeature, WireInstallation,
    WireInventory, WireRecipe, WireTarget,
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
        if configuration.executable_names != ["SyntheticEditor.exe"] {
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
}

fn main() -> std::io::Result<()> {
    serve_stdio(SyntheticController::default())
}

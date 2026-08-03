//! Versioned stdio protocol and authoring surface for isolated Controller Plugins.

use serde::{Deserialize, Serialize};
use std::io::{self, BufRead, Write};

pub const PROTOCOL_SCHEMA: &str = "glyphshift.controller/2";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WireFeature {
    TextObserve,
    TextReplace,
    FontSubstitute,
    LayoutAdjust,
    ResourceReplace,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestEnvelope {
    pub schema: String,
    pub request_id: u64,
    #[serde(flatten)]
    pub request: Request,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Request {
    Handshake {
        extension_id: String,
        version_major: u16,
        version_minor: u16,
        nonce: [u8; 32],
        configuration: WireControllerConfiguration,
    },
    Inventory,
    Launch {
        installation_token: String,
    },
    Prepare {
        target_token: String,
        requested_features: Vec<WireFeature>,
    },
    ActivateRuntime {
        target_token: String,
        deployment: WireRuntimeDeployment,
    },
    UpdateRuntime {
        target_token: String,
        publication_json: String,
        generation: u64,
    },
    ControlCapture {
        target_token: String,
        paused: bool,
    },
    ControlDiagnostics {
        target_token: String,
        enabled: bool,
    },
    QueryDiagnostics {
        target_token: String,
    },
    DeactivateRuntime {
        target_token: String,
    },
    Cancel {
        operation: WireOperation,
    },
    Terminate,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WireOperation {
    Inventory,
    Prepare { target_token: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResponseEnvelope {
    pub schema: String,
    pub request_id: u64,
    #[serde(flatten)]
    pub response: Response,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Response {
    Hello {
        extension_id: String,
        version_major: u16,
        version_minor: u16,
        nonce: [u8; 32],
    },
    Inventory {
        inventory: WireInventory,
    },
    LaunchAccepted,
    Recipe {
        recipe: WireRecipe,
    },
    RuntimeActivated {
        generation: u64,
        publication_identity: [u8; 32],
    },
    RuntimeUpdated {
        generation: u64,
        publication_identity: [u8; 32],
    },
    CaptureControlled {
        paused: bool,
    },
    DiagnosticsControlled {
        enabled: bool,
    },
    RuntimeDiagnostics {
        batch: WireRuntimeTraceBatch,
    },
    RuntimeDeactivated,
    Cancelled,
    Error {
        code: String,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireInventory {
    pub installations: Vec<WireInstallation>,
    pub targets: Vec<WireTarget>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireControllerConfiguration {
    pub executable_names: Vec<String>,
    #[serde(default)]
    pub executable_paths: Vec<String>,
    #[serde(default)]
    pub descendant_executable_names: Vec<String>,
    pub adapter_requirements: Vec<WireAdapterRequirement>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireInstallation {
    pub token: String,
    pub display_name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireTarget {
    pub token: String,
    pub display_name: String,
    pub operating_system: String,
    pub architecture: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireRuntimeDeployment {
    pub runtime_library: String,
    pub runtime_library_sha256: [u8; 32],
    pub deployment_json: String,
    pub generation: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireRecipe {
    pub adapters: Vec<WireAdapterRequirement>,
    pub controller_loss_policy: WireControllerLossPolicy,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireAdapterRequirement {
    pub adapter_id: String,
    pub version_major: u16,
    pub version_minor: u16,
    pub version_patch: u16,
    pub features: Vec<WireFeature>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WireControllerLossPolicy {
    Continue,
    Degrade,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WireRuntimeAck {
    pub generation: u64,
    pub publication_identity: [u8; 32],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WireRuntimeTraceStatus {
    NoMatch,
    Matched,
    ContextRecorded,
    InvalidObservation,
    InvalidRouteProgram,
    ExecutionLimitExceeded,
    StateLimitExceeded,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WireRuntimeTextOutcome {
    Unmatched,
    Replaced,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WireRuntimeFontOutcome {
    Unmatched,
    Protected,
    Substituted,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireRuntimeTraceRecord {
    pub adapter_id: String,
    pub source_text: String,
    pub status: WireRuntimeTraceStatus,
    pub text: WireRuntimeTextOutcome,
    pub font: WireRuntimeFontOutcome,
    pub generation: u64,
    pub publication_identity: [u8; 32],
    pub translation_digest: [u8; 32],
    pub font_policy_digest: [u8; 32],
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireRuntimeTraceBatch {
    pub records: Vec<WireRuntimeTraceRecord>,
    pub dropped: u64,
}

pub trait ControllerPlugin {
    fn configure(
        &mut self,
        _extension_id: &str,
        _configuration: &WireControllerConfiguration,
    ) -> Result<(), PluginError> {
        Ok(())
    }

    fn inventory(&mut self) -> Result<WireInventory, PluginError>;

    fn launch(&mut self, installation_token: &str) -> Result<(), PluginError>;

    fn prepare(
        &mut self,
        target_token: &str,
        requested_features: &[WireFeature],
    ) -> Result<WireRecipe, PluginError>;

    fn activate_runtime(
        &mut self,
        _target_token: &str,
        _deployment: &WireRuntimeDeployment,
    ) -> Result<WireRuntimeAck, PluginError> {
        Err(PluginError::new("runtime_activation_unsupported"))
    }

    fn update_runtime(
        &mut self,
        _target_token: &str,
        _publication_json: &str,
        _generation: u64,
    ) -> Result<WireRuntimeAck, PluginError> {
        Err(PluginError::new("runtime_update_unsupported"))
    }

    fn control_capture(&mut self, _target_token: &str, _paused: bool) -> Result<(), PluginError> {
        Err(PluginError::new("capture_control_unsupported"))
    }

    fn control_diagnostics(
        &mut self,
        _target_token: &str,
        _enabled: bool,
    ) -> Result<(), PluginError> {
        Err(PluginError::new("runtime_diagnostics_unsupported"))
    }

    fn query_diagnostics(
        &mut self,
        _target_token: &str,
    ) -> Result<WireRuntimeTraceBatch, PluginError> {
        Err(PluginError::new("runtime_diagnostics_unsupported"))
    }

    fn deactivate_runtime(&mut self, _target_token: &str) -> Result<(), PluginError> {
        Err(PluginError::new("runtime_deactivation_unsupported"))
    }

    fn cancel(&mut self, _operation: &WireOperation) -> Result<(), PluginError> {
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginError {
    code: String,
}

impl PluginError {
    #[must_use]
    pub fn new(code: impl Into<String>) -> Self {
        Self { code: code.into() }
    }
}

pub fn serve_stdio(plugin: impl ControllerPlugin) -> io::Result<()> {
    serve(
        plugin,
        io::BufReader::new(io::stdin().lock()),
        io::stdout().lock(),
    )
}

pub fn serve(
    mut plugin: impl ControllerPlugin,
    input: impl BufRead,
    mut output: impl Write,
) -> io::Result<()> {
    for line in input.lines() {
        let line = line?;
        let request = match serde_json::from_str::<RequestEnvelope>(&line) {
            Ok(request) if request.schema == PROTOCOL_SCHEMA => request,
            Ok(_) | Err(_) => continue,
        };
        let request_id = request.request_id;
        let response = match request.request {
            Request::Handshake {
                extension_id,
                version_major,
                version_minor,
                nonce,
                configuration,
            } => match plugin.configure(&extension_id, &configuration) {
                Ok(()) => Response::Hello {
                    extension_id,
                    version_major,
                    version_minor,
                    nonce,
                },
                Err(error) => plugin_error(error),
            },
            Request::Inventory => plugin
                .inventory()
                .map(|inventory| Response::Inventory { inventory })
                .unwrap_or_else(plugin_error),
            Request::Launch { installation_token } => plugin
                .launch(&installation_token)
                .map(|()| Response::LaunchAccepted)
                .unwrap_or_else(plugin_error),
            Request::Prepare {
                target_token,
                requested_features,
            } => plugin
                .prepare(&target_token, &requested_features)
                .map(|recipe| Response::Recipe { recipe })
                .unwrap_or_else(plugin_error),
            Request::ActivateRuntime {
                target_token,
                deployment,
            } => plugin
                .activate_runtime(&target_token, &deployment)
                .map(|ack| Response::RuntimeActivated {
                    generation: ack.generation,
                    publication_identity: ack.publication_identity,
                })
                .unwrap_or_else(plugin_error),
            Request::UpdateRuntime {
                target_token,
                publication_json,
                generation,
            } => plugin
                .update_runtime(&target_token, &publication_json, generation)
                .map(|ack| Response::RuntimeUpdated {
                    generation: ack.generation,
                    publication_identity: ack.publication_identity,
                })
                .unwrap_or_else(plugin_error),
            Request::ControlCapture {
                target_token,
                paused,
            } => plugin
                .control_capture(&target_token, paused)
                .map(|()| Response::CaptureControlled { paused })
                .unwrap_or_else(plugin_error),
            Request::ControlDiagnostics {
                target_token,
                enabled,
            } => plugin
                .control_diagnostics(&target_token, enabled)
                .map(|()| Response::DiagnosticsControlled { enabled })
                .unwrap_or_else(plugin_error),
            Request::QueryDiagnostics { target_token } => plugin
                .query_diagnostics(&target_token)
                .map(|batch| Response::RuntimeDiagnostics { batch })
                .unwrap_or_else(plugin_error),
            Request::DeactivateRuntime { target_token } => plugin
                .deactivate_runtime(&target_token)
                .map(|()| Response::RuntimeDeactivated)
                .unwrap_or_else(plugin_error),
            Request::Cancel { operation } => plugin
                .cancel(&operation)
                .map(|()| Response::Cancelled)
                .unwrap_or_else(plugin_error),
            Request::Terminate => break,
        };
        serde_json::to_writer(
            &mut output,
            &ResponseEnvelope {
                schema: PROTOCOL_SCHEMA.into(),
                request_id,
                response,
            },
        )?;
        output.write_all(b"\n")?;
        output.flush()?;
    }
    Ok(())
}

fn plugin_error(error: PluginError) -> Response {
    Response::Error { code: error.code }
}

use crate::platform::{
    enumerate_processes, file_sha256, has_authorized_ancestor, validate_executable_name,
    validate_executable_path,
};
#[cfg(windows)]
use crate::remote;
use glyphshift_controller_sdk::{
    ControllerPlugin, PluginError, WireAdapterRequirement, WireCaptureObservationBatch,
    WireCaptureObservationRecord, WireCaptureTranslationContext, WireControllerConfiguration, WireControllerLossPolicy,
    WireFeature, WireInventory, WireRecipe, WireRuntimeAck, WireRuntimeDeployment,
    WireRuntimeFontOutcome, WireRuntimeTextOutcome, WireRuntimeTraceBatch, WireRuntimeTraceRecord,
    WireRuntimeTraceStatus, WireTarget, WireWorkerTargetGrant,
};
use glyphshift_runtime_contract::RuntimePublication;
use glyphshift_target_runtime_contract::TargetRuntimeDeployment;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

#[derive(Clone, Debug)]
pub(super) struct ProcessRecord {
    pub(super) process_id: u32,
    pub(super) parent_process_id: u32,
    pub(super) started_at: Option<u64>,
    pub(super) executable_name: String,
    pub(super) executable_path: Option<String>,
    pub(super) architecture: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct ProcessInstanceId {
    process_id: u32,
    started_at: u64,
}

impl ProcessRecord {
    fn instance_id(&self) -> Option<ProcessInstanceId> {
        self.started_at.map(|started_at| ProcessInstanceId {
            process_id: self.process_id,
            started_at,
        })
    }
}

pub(super) trait ProcessInventory {
    fn snapshot(&mut self) -> Result<Vec<ProcessRecord>, PluginError>;
}

struct SystemProcessInventory;

impl ProcessInventory for SystemProcessInventory {
    fn snapshot(&mut self) -> Result<Vec<ProcessRecord>, PluginError> {
        enumerate_processes()
    }
}

#[derive(Clone, Debug)]
pub(super) struct AuthorizedProcess {
    pub(super) process: ProcessRecord,
    pub(super) is_root: bool,
}

pub struct WindowsController {
    executable_names: BTreeSet<String>,
    executable_paths: BTreeSet<String>,
    descendant_executable_names: BTreeSet<String>,
    adapter_requirements: Vec<WireAdapterRequirement>,
    process_inventory: Box<dyn ProcessInventory>,
    admitted_instances: BTreeSet<ProcessInstanceId>,
    pub(super) targets: BTreeMap<String, ProcessRecord>,
    pub(super) runtime_libraries: BTreeMap<String, std::path::PathBuf>,
    next_target_token: u64,
}

impl Default for WindowsController {
    fn default() -> Self {
        Self::new()
    }
}

impl WindowsController {
    #[must_use]
    pub fn new() -> Self {
        Self::with_process_inventory(Box::new(SystemProcessInventory))
    }

    pub(super) fn with_process_inventory(process_inventory: Box<dyn ProcessInventory>) -> Self {
        Self {
            executable_names: BTreeSet::new(),
            executable_paths: BTreeSet::new(),
            descendant_executable_names: BTreeSet::new(),
            adapter_requirements: Vec::new(),
            process_inventory,
            admitted_instances: BTreeSet::new(),
            targets: BTreeMap::new(),
            runtime_libraries: BTreeMap::new(),
            next_target_token: 1,
        }
    }

    fn is_root(&self, process: &ProcessRecord) -> bool {
        if self.executable_paths.is_empty() {
            self.executable_names
                .contains(&process.executable_name.to_lowercase())
        } else {
            process
                .executable_path
                .as_ref()
                .is_some_and(|path| self.executable_paths.contains(path))
        }
    }

    pub(super) fn authorized_processes(
        &mut self,
        processes: Vec<ProcessRecord>,
    ) -> Vec<AuthorizedProcess> {
        let current_instances = processes
            .iter()
            .filter_map(ProcessRecord::instance_id)
            .collect::<BTreeSet<_>>();
        self.admitted_instances
            .retain(|instance| current_instances.contains(instance));

        let by_process_id = processes
            .iter()
            .map(|process| {
                (
                    process.process_id,
                    (process.parent_process_id, process.instance_id()),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let root_candidates = processes
            .iter()
            .filter(|process| process.instance_id().is_some() && self.is_root(process))
            .filter_map(|process| {
                process
                    .instance_id()
                    .map(|instance| (instance, process.parent_process_id))
            })
            .collect::<Vec<_>>();
        let mut root_anchors = self.admitted_instances.clone();
        root_anchors.extend(root_candidates.iter().map(|(instance, _)| *instance));
        let roots = root_candidates
            .iter()
            .filter(|(_, parent_process_id)| {
                !has_authorized_ancestor(*parent_process_id, &by_process_id, &root_anchors)
            })
            .map(|(instance, _)| *instance)
            .collect::<BTreeSet<_>>();
        let matching_root_executables = root_candidates
            .iter()
            .map(|(instance, _)| *instance)
            .collect::<BTreeSet<_>>();
        let mut anchors = self.admitted_instances.clone();
        anchors.extend(roots.iter().copied());

        let mut authorized = Vec::new();
        for process in processes {
            let Some(instance) = process.instance_id() else {
                continue;
            };
            let is_root = roots.contains(&instance);
            let is_admitted = self.admitted_instances.contains(&instance);
            let is_same_executable_descendant = matching_root_executables.contains(&instance)
                && !is_root
                && has_authorized_ancestor(process.parent_process_id, &by_process_id, &anchors);
            let is_allowed_descendant = self
                .descendant_executable_names
                .contains(&process.executable_name.to_lowercase())
                && has_authorized_ancestor(process.parent_process_id, &by_process_id, &anchors);
            if is_root || is_admitted || is_same_executable_descendant || is_allowed_descendant {
                authorized.push(AuthorizedProcess { process, is_root });
                self.admitted_instances.insert(instance);
            }
        }
        authorized.sort_by(|left, right| {
            right
                .is_root
                .cmp(&left.is_root)
                .then(
                    left.process
                        .executable_name
                        .to_lowercase()
                        .cmp(&right.process.executable_name.to_lowercase()),
                )
                .then(left.process.process_id.cmp(&right.process.process_id))
        });
        authorized
    }

    fn rebuild_targets(&mut self, processes: Vec<AuthorizedProcess>) -> Vec<WireTarget> {
        let existing_tokens = self
            .targets
            .iter()
            .filter_map(|(token, process)| process.instance_id().map(|instance| (instance, token)))
            .collect::<BTreeMap<_, _>>();
        let mut targets = BTreeMap::new();
        let mut display_counts = BTreeMap::<String, usize>::new();
        let mut wire_targets = Vec::new();
        for authorized in processes {
            let process = authorized.process;
            let instance = process
                .instance_id()
                .expect("authorized processes have a stable instance identity");
            let token = if let Some(token) = existing_tokens.get(&instance) {
                (*token).clone()
            } else {
                let token = format!("target:{}", self.next_target_token);
                self.next_target_token = self.next_target_token.saturating_add(1);
                token
            };
            let base_name = Path::new(&process.executable_name)
                .file_stem()
                .and_then(|value| value.to_str())
                .unwrap_or("Windows 软件")
                .to_string();
            let count = display_counts.entry(base_name.clone()).or_default();
            *count += 1;
            let display_name = if *count == 1 {
                format!("{base_name} · 运行中")
            } else {
                format!("{base_name} · 运行实例 {}", *count)
            };
            wire_targets.push(WireTarget {
                token: token.clone(),
                display_name,
                operating_system: "windows".into(),
                architecture: process.architecture.clone(),
            });
            targets.insert(token, process);
        }
        self.targets = targets;
        self.runtime_libraries
            .retain(|token, _| self.targets.contains_key(token));
        wire_targets
    }
}

impl ControllerPlugin for WindowsController {
    fn configure(
        &mut self,
        _extension_id: &str,
        configuration: &WireControllerConfiguration,
    ) -> Result<(), PluginError> {
        let executable_names = configuration
            .executable_names
            .iter()
            .map(|name| validate_executable_name(name))
            .collect::<Result<BTreeSet<_>, _>>()?;
        let executable_paths = configuration
            .executable_paths
            .iter()
            .map(|path| validate_executable_path(path))
            .collect::<Result<BTreeSet<_>, _>>()?;
        let descendant_executable_names = configuration
            .descendant_executable_names
            .iter()
            .map(|name| validate_executable_name(name))
            .collect::<Result<BTreeSet<_>, _>>()?;
        if executable_names.is_empty() && executable_paths.is_empty() {
            return Err(PluginError::new("target_configuration_empty"));
        }
        self.executable_names = executable_names;
        self.executable_paths = executable_paths;
        self.descendant_executable_names = descendant_executable_names;
        self.adapter_requirements = configuration.adapter_requirements.clone();
        self.admitted_instances.clear();
        self.targets.clear();
        self.runtime_libraries.clear();
        self.next_target_token = 1;
        Ok(())
    }

    fn inventory(&mut self) -> Result<WireInventory, PluginError> {
        let processes = self.process_inventory.snapshot()?;
        let authorized = self.authorized_processes(processes);
        let targets = self.rebuild_targets(authorized);
        Ok(WireInventory {
            installations: Vec::new(),
            targets,
        })
    }

    fn launch(&mut self, _installation_token: &str) -> Result<(), PluginError> {
        Err(PluginError::new("launch_not_configured"))
    }

    fn prepare(
        &mut self,
        target_token: &str,
        requested_features: &[WireFeature],
    ) -> Result<WireRecipe, PluginError> {
        if !self.targets.contains_key(target_token) {
            return Err(PluginError::new("target_not_found"));
        }
        let adapters = self
            .adapter_requirements
            .iter()
            .filter_map(|requirement| {
                let mut requirement = requirement.clone();
                requirement
                    .features
                    .retain(|feature| requested_features.contains(feature));
                (!requirement.features.is_empty()).then_some(requirement)
            })
            .collect();
        Ok(WireRecipe {
            adapters,
            controller_loss_policy: WireControllerLossPolicy::Degrade,
        })
    }

    fn authorize_worker_target(
        &mut self,
        target_token: &str,
    ) -> Result<WireWorkerTargetGrant, PluginError> {
        let target = self
            .targets
            .get(target_token)
            .ok_or_else(|| PluginError::new("target_not_found"))?;
        let started_at = target
            .started_at
            .ok_or_else(|| PluginError::new("target_instance_unavailable"))?;
        Ok(WireWorkerTargetGrant {
            platform: "windows-process-v1".into(),
            payload: format!("{}:{started_at}", target.process_id),
        })
    }

    fn activate_runtime(
        &mut self,
        target_token: &str,
        deployment: &WireRuntimeDeployment,
    ) -> Result<WireRuntimeAck, PluginError> {
        let target = self
            .targets
            .get(target_token)
            .ok_or_else(|| PluginError::new("target_not_found"))?;
        let runtime_library = std::path::PathBuf::from(&deployment.runtime_library);
        let decoded = TargetRuntimeDeployment::decode_json(&deployment.deployment_json)
            .map_err(|_| PluginError::new("invalid_runtime_deployment"))?;
        if decoded.publication().generation().value() != deployment.generation
            || !runtime_library.is_absolute()
            || !runtime_library.is_file()
            || file_sha256(&runtime_library).as_ref() != Ok(&deployment.runtime_library_sha256)
            || decoded
                .adapters()
                .iter()
                .any(|adapter| !adapter.library().is_absolute() || !adapter.library().is_file())
        {
            return Err(PluginError::new("invalid_runtime_deployment"));
        }
        if target.architecture != std::env::consts::ARCH
            || crate::platform::process_architecture(target.process_id) != target.architecture
            || target.started_at.is_none()
            || crate::platform::process_started_at(target.process_id) != target.started_at
            || crate::platform::process_executable_path(target.process_id) != target.executable_path
        {
            return Err(PluginError::new("target_architecture_or_instance_changed"));
        }
        if glyphshift_adapter_native_host::inspect_pe_architecture(&runtime_library).ok()
            != Some(target.architecture.as_str())
            || decoded.adapters().iter().any(|adapter| {
                glyphshift_adapter_native_host::inspect_pe_architecture(adapter.library()).ok()
                    != Some(target.architecture.as_str())
            })
        {
            return Err(PluginError::new("invalid_runtime_deployment"));
        }
        let publication_identity = decoded
            .publication()
            .identity()
            .map_err(|_| PluginError::new("invalid_runtime_deployment"))?
            .as_bytes();
        let activation = remote::activate(target, &runtime_library, &deployment.deployment_json)
            .map_err(|error| {
                PluginError::new(format!("runtime_activation_failed:{}", error.code()))
            })?;
        self.runtime_libraries
            .insert(target_token.into(), runtime_library);
        Ok(WireRuntimeAck {
            generation: deployment.generation,
            publication_identity,
            active_adapter_ids: Some(activation.active_adapter_ids().map(Into::into).collect()),
        })
    }

    fn update_runtime(
        &mut self,
        target_token: &str,
        publication_json: &str,
        generation: u64,
    ) -> Result<WireRuntimeAck, PluginError> {
        let target = self
            .targets
            .get(target_token)
            .ok_or_else(|| PluginError::new("target_not_found"))?;
        let runtime_library = self
            .runtime_libraries
            .get(target_token)
            .ok_or_else(|| PluginError::new("runtime_not_active"))?;
        let publication = RuntimePublication::decode_json(publication_json)
            .map_err(|_| PluginError::new("invalid_runtime_publication"))?;
        if publication.generation().value() != generation {
            return Err(PluginError::new("invalid_runtime_publication"));
        }
        let publication_identity = publication
            .identity()
            .map_err(|_| PluginError::new("invalid_runtime_publication"))?
            .as_bytes();
        remote::update(target, runtime_library, publication_json)
            .map_err(|error| PluginError::new(format!("runtime_update_failed:{}", error.code())))?;
        Ok(WireRuntimeAck {
            generation,
            publication_identity,
            active_adapter_ids: None,
        })
    }

    fn control_capture(&mut self, target_token: &str, paused: bool) -> Result<(), PluginError> {
        let target = self
            .targets
            .get(target_token)
            .ok_or_else(|| PluginError::new("unknown_target"))?;
        let runtime_library = self
            .runtime_libraries
            .get(target_token)
            .ok_or_else(|| PluginError::new("runtime_not_active"))?;
        remote::control_capture(target, runtime_library, paused)
            .map_err(|error| PluginError::new(format!("capture_control_failed:{}", error.code())))
    }

    fn control_diagnostics(
        &mut self,
        target_token: &str,
        enabled: bool,
    ) -> Result<(), PluginError> {
        let target = self
            .targets
            .get(target_token)
            .ok_or_else(|| PluginError::new("unknown_target"))?;
        let runtime_library = self
            .runtime_libraries
            .get(target_token)
            .ok_or_else(|| PluginError::new("runtime_not_active"))?;
        remote::control_diagnostics(target, runtime_library, enabled).map_err(|error| {
            PluginError::new(format!("runtime_diagnostics_failed:{}", error.code()))
        })
    }

    fn query_diagnostics(
        &mut self,
        target_token: &str,
    ) -> Result<WireRuntimeTraceBatch, PluginError> {
        let target = self
            .targets
            .get(target_token)
            .ok_or_else(|| PluginError::new("unknown_target"))?;
        let runtime_library = self
            .runtime_libraries
            .get(target_token)
            .ok_or_else(|| PluginError::new("runtime_not_active"))?;
        let batch = remote::query_diagnostics(target, runtime_library).map_err(|error| {
            PluginError::new(format!("runtime_diagnostics_failed:{}", error.code()))
        })?;
        Ok(WireRuntimeTraceBatch {
            records: batch
                .records()
                .iter()
                .map(|record| WireRuntimeTraceRecord {
                    adapter_id: record.adapter_id().into(),
                    source_text: record.source_text().into(),
                    status: match record.status() {
                        glyphshift_target_runtime_contract::RuntimeTraceStatus::NoMatch => {
                            WireRuntimeTraceStatus::NoMatch
                        }
                        glyphshift_target_runtime_contract::RuntimeTraceStatus::Matched => {
                            WireRuntimeTraceStatus::Matched
                        }
                        glyphshift_target_runtime_contract::RuntimeTraceStatus::ContextRecorded => {
                            WireRuntimeTraceStatus::ContextRecorded
                        }
                        glyphshift_target_runtime_contract::RuntimeTraceStatus::InvalidObservation => {
                            WireRuntimeTraceStatus::InvalidObservation
                        }
                        glyphshift_target_runtime_contract::RuntimeTraceStatus::InvalidRouteProgram => {
                            WireRuntimeTraceStatus::InvalidRouteProgram
                        }
                        glyphshift_target_runtime_contract::RuntimeTraceStatus::ExecutionLimitExceeded => {
                            WireRuntimeTraceStatus::ExecutionLimitExceeded
                        }
                        glyphshift_target_runtime_contract::RuntimeTraceStatus::StateLimitExceeded => {
                            WireRuntimeTraceStatus::StateLimitExceeded
                        }
                    },
                    text: match record.text() {
                        glyphshift_target_runtime_contract::RuntimeTextOutcome::Unmatched => {
                            WireRuntimeTextOutcome::Unmatched
                        }
                        glyphshift_target_runtime_contract::RuntimeTextOutcome::Replaced => {
                            WireRuntimeTextOutcome::Replaced
                        }
                    },
                    font: match record.font() {
                        glyphshift_target_runtime_contract::RuntimeFontOutcome::Unmatched => {
                            WireRuntimeFontOutcome::Unmatched
                        }
                        glyphshift_target_runtime_contract::RuntimeFontOutcome::Protected => {
                            WireRuntimeFontOutcome::Protected
                        }
                        glyphshift_target_runtime_contract::RuntimeFontOutcome::Substituted => {
                            WireRuntimeFontOutcome::Substituted
                        }
                    },
                    generation: record.generation(),
                    publication_identity: record.publication_identity(),
                    translation_digest: record.translation_digest(),
                    font_policy_digest: record.font_policy_digest(),
                })
                .collect(),
            dropped: batch.dropped(),
        })
    }

    fn query_observations(
        &mut self,
        target_token: &str,
    ) -> Result<WireCaptureObservationBatch, PluginError> {
        let target = self
            .targets
            .get(target_token)
            .ok_or_else(|| PluginError::new("unknown_target"))?;
        let runtime_library = self
            .runtime_libraries
            .get(target_token)
            .ok_or_else(|| PluginError::new("runtime_not_active"))?;
        let batch = remote::query_observations(target, runtime_library).map_err(|error| {
            PluginError::new(format!("runtime_observations_failed:{}", error.code()))
        })?;
        Ok(WireCaptureObservationBatch {
            producer_id: batch.producer_id().as_str().into(),
            generation: batch.generation(),
            dropped_total: batch.dropped_total(),
            records: batch
                .records()
                .iter()
                .map(|record| WireCaptureObservationRecord {
                    sequence: record.sequence(),
                    adapter_id: record.adapter_id().into(),
                    source: record.source().into(),
                    translation_context: record.translation_context().map(|context| {
                        WireCaptureTranslationContext {
                            context: context.context().map(str::to_owned),
                            disambiguation: context.disambiguation().map(str::to_owned),
                            plural_n: context.plural_n(),
                        }
                    }),
                })
                .collect(),
        })
    }

    fn deactivate_runtime(&mut self, target_token: &str) -> Result<(), PluginError> {
        let target = self
            .targets
            .get(target_token)
            .ok_or_else(|| PluginError::new("target_not_found"))?;
        let runtime_library = self
            .runtime_libraries
            .get(target_token)
            .ok_or_else(|| PluginError::new("runtime_not_active"))?;
        remote::deactivate(target, runtime_library).map_err(|error| {
            PluginError::new(format!("runtime_deactivation_failed:{}", error.code()))
        })
    }
}

//! Generic Windows target discovery behind the isolated Controller protocol.

use glyphshift_controller_sdk::{
    ControllerPlugin, PluginError, WireAdapterRequirement, WireControllerConfiguration,
    WireControllerLossPolicy, WireFeature, WireInventory, WireRecipe, WireRuntimeAck,
    WireRuntimeDeployment, WireRuntimeFontOutcome, WireRuntimeTextOutcome, WireRuntimeTraceBatch,
    WireRuntimeTraceRecord, WireRuntimeTraceStatus, WireTarget,
};
use glyphshift_runtime_contract::RuntimePublication;
use glyphshift_target_runtime_contract::TargetRuntimeDeployment;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::path::Path;

#[derive(Clone, Debug)]
struct ProcessRecord {
    process_id: u32,
    executable_name: String,
    executable_path: Option<String>,
    architecture: String,
}

#[derive(Default)]
pub struct WindowsController {
    executable_names: BTreeSet<String>,
    executable_paths: BTreeSet<String>,
    adapter_requirements: Vec<WireAdapterRequirement>,
    targets: BTreeMap<String, ProcessRecord>,
    runtime_libraries: BTreeMap<String, std::path::PathBuf>,
}

impl WindowsController {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            executable_names: BTreeSet::new(),
            executable_paths: BTreeSet::new(),
            adapter_requirements: Vec::new(),
            targets: BTreeMap::new(),
            runtime_libraries: BTreeMap::new(),
        }
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
        if executable_names.is_empty() && executable_paths.is_empty() {
            return Err(PluginError::new("target_configuration_empty"));
        }
        self.executable_names = executable_names;
        self.executable_paths = executable_paths;
        self.adapter_requirements = configuration.adapter_requirements.clone();
        self.targets.clear();
        self.runtime_libraries.clear();
        Ok(())
    }

    fn inventory(&mut self) -> Result<WireInventory, PluginError> {
        let mut processes = enumerate_processes()?
            .into_iter()
            .filter(|process| {
                if self.executable_paths.is_empty() {
                    self.executable_names
                        .contains(&process.executable_name.to_lowercase())
                } else {
                    process
                        .executable_path
                        .as_ref()
                        .is_some_and(|path| self.executable_paths.contains(path))
                }
            })
            .collect::<Vec<_>>();
        processes.sort_by(|left, right| {
            left.executable_name
                .to_lowercase()
                .cmp(&right.executable_name.to_lowercase())
                .then(left.process_id.cmp(&right.process_id))
        });
        self.targets.clear();
        let mut display_counts = BTreeMap::<String, usize>::new();
        let targets = processes
            .into_iter()
            .enumerate()
            .map(|(index, process)| {
                let token = format!("target:{}", index + 1);
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
                let target = WireTarget {
                    token: token.clone(),
                    display_name,
                    operating_system: "windows".into(),
                    architecture: process.architecture.clone(),
                };
                self.targets.insert(token, process);
                target
            })
            .collect();
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
        let publication_identity = decoded
            .publication()
            .identity()
            .map_err(|_| PluginError::new("invalid_runtime_deployment"))?
            .as_bytes();
        remote::activate(
            target.process_id,
            &runtime_library,
            &deployment.deployment_json,
        )
        .map_err(|error| PluginError::new(format!("runtime_activation_failed:{}", error.code())))?;
        self.runtime_libraries
            .insert(target_token.into(), runtime_library);
        Ok(WireRuntimeAck {
            generation: deployment.generation,
            publication_identity,
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
        remote::update(target.process_id, runtime_library, publication_json)
            .map_err(|error| PluginError::new(format!("runtime_update_failed:{}", error.code())))?;
        Ok(WireRuntimeAck {
            generation,
            publication_identity,
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
        remote::control_capture(target.process_id, runtime_library, paused)
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
        remote::control_diagnostics(target.process_id, runtime_library, enabled).map_err(|error| {
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
        let batch =
            remote::query_diagnostics(target.process_id, runtime_library).map_err(|error| {
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

    fn deactivate_runtime(&mut self, target_token: &str) -> Result<(), PluginError> {
        let target = self
            .targets
            .get(target_token)
            .ok_or_else(|| PluginError::new("target_not_found"))?;
        let runtime_library = self
            .runtime_libraries
            .get(target_token)
            .ok_or_else(|| PluginError::new("runtime_not_active"))?;
        remote::deactivate(target.process_id, runtime_library).map_err(|error| {
            PluginError::new(format!("runtime_deactivation_failed:{}", error.code()))
        })
    }
}

fn file_sha256(path: &Path) -> Result<[u8; 32], ()> {
    let mut file = File::open(path).map_err(|_| ())?;
    let mut digest = Sha256::new();
    std::io::copy(&mut file, &mut digest).map_err(|_| ())?;
    Ok(digest.finalize().into())
}

#[cfg(windows)]
mod remote;

fn validate_executable_name(name: &str) -> Result<String, PluginError> {
    let trimmed = name.trim();
    if trimmed.is_empty()
        || Path::new(trimmed)
            .file_name()
            .and_then(|value| value.to_str())
            != Some(trimmed)
        || trimmed.contains(['/', '\\'])
    {
        return Err(PluginError::new("invalid_executable_name"));
    }
    Ok(trimmed.to_lowercase())
}

fn validate_executable_path(path: &str) -> Result<String, PluginError> {
    let trimmed = path.trim();
    let path = Path::new(trimmed);
    if trimmed.is_empty()
        || !path.is_absolute()
        || path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_none_or(|extension| !extension.eq_ignore_ascii_case("exe"))
    {
        return Err(PluginError::new("invalid_executable_path"));
    }
    Ok(normalize_executable_path(trimmed))
}

fn normalize_executable_path(path: &str) -> String {
    path.strip_prefix(r"\\?\")
        .unwrap_or(path)
        .replace('/', "\\")
        .to_lowercase()
}

#[cfg(windows)]
fn enumerate_processes() -> Result<Vec<ProcessRecord>, PluginError> {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };

    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot == INVALID_HANDLE_VALUE {
        return Err(PluginError::new("process_inventory_unavailable"));
    }
    let mut entry = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    let mut processes = Vec::new();
    let mut available = unsafe { Process32FirstW(snapshot, &mut entry) } != 0;
    while available {
        let length = entry
            .szExeFile
            .iter()
            .position(|value| *value == 0)
            .unwrap_or(entry.szExeFile.len());
        let executable_name = String::from_utf16_lossy(&entry.szExeFile[..length]);
        processes.push(ProcessRecord {
            process_id: entry.th32ProcessID,
            executable_name,
            executable_path: process_executable_path(entry.th32ProcessID),
            architecture: process_architecture(entry.th32ProcessID),
        });
        available = unsafe { Process32NextW(snapshot, &mut entry) } != 0;
    }
    unsafe {
        CloseHandle(snapshot);
    }
    Ok(processes)
}

#[cfg(windows)]
fn process_executable_path(process_id: u32) -> Option<String> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, process_id) };
    if process.is_null() {
        return None;
    }
    let mut buffer = vec![0_u16; 32_768];
    let mut length = buffer.len() as u32;
    let queried =
        unsafe { QueryFullProcessImageNameW(process, 0, buffer.as_mut_ptr(), &mut length) };
    unsafe {
        CloseHandle(process);
    }
    if queried == 0 || length == 0 {
        return None;
    }
    Some(normalize_executable_path(&String::from_utf16_lossy(
        &buffer[..length as usize],
    )))
}

#[cfg(windows)]
fn process_architecture(process_id: u32) -> String {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::SystemInformation::{
        IMAGE_FILE_MACHINE_AMD64, IMAGE_FILE_MACHINE_I386, IMAGE_FILE_MACHINE_UNKNOWN,
    };
    use windows_sys::Win32::System::Threading::{
        IsWow64Process2, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, process_id) };
    if process.is_null() {
        return std::env::consts::ARCH.into();
    }
    let mut process_machine = IMAGE_FILE_MACHINE_UNKNOWN;
    let mut native_machine = IMAGE_FILE_MACHINE_UNKNOWN;
    let queried =
        unsafe { IsWow64Process2(process, &mut process_machine, &mut native_machine) } != 0;
    unsafe {
        CloseHandle(process);
    }
    if !queried {
        return std::env::consts::ARCH.into();
    }
    match if process_machine == IMAGE_FILE_MACHINE_UNKNOWN {
        native_machine
    } else {
        process_machine
    } {
        IMAGE_FILE_MACHINE_I386 => "x86".into(),
        IMAGE_FILE_MACHINE_AMD64 => "x86_64".into(),
        _ => "unknown".into(),
    }
}

#[cfg(not(windows))]
fn enumerate_processes() -> Result<Vec<ProcessRecord>, PluginError> {
    Err(PluginError::new("unsupported_operating_system"))
}

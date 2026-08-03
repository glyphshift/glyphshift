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
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WindowsExecutable {
    path: PathBuf,
    name: Box<str>,
    architecture: Box<str>,
    running: bool,
}

impl WindowsExecutable {
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn architecture(&self) -> &str {
        &self.architecture
    }

    #[must_use]
    pub const fn running(&self) -> bool {
        self.running
    }
}

pub fn inspect_windows_executable(
    path: impl AsRef<Path>,
) -> Result<WindowsExecutable, PluginError> {
    windows_executable(path.as_ref(), None)
}

#[cfg(windows)]
pub fn foreground_windows_executable() -> Result<WindowsExecutable, PluginError> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowThreadProcessId,
    };

    let window = unsafe { GetForegroundWindow() };
    if window.is_null() {
        return Err(PluginError::new("foreground_window_unavailable"));
    }
    let mut process_id = 0_u32;
    unsafe {
        GetWindowThreadProcessId(window, &mut process_id);
    }
    if process_id == 0 {
        return Err(PluginError::new("foreground_process_unavailable"));
    }
    let path = process_executable_path(process_id)
        .ok_or_else(|| PluginError::new("foreground_process_inaccessible"))?;
    windows_executable(Path::new(&path), Some(true))
}

#[cfg(not(windows))]
pub fn foreground_windows_executable() -> Result<WindowsExecutable, PluginError> {
    Err(PluginError::new("unsupported_operating_system"))
}

#[derive(Clone, Debug)]
struct ProcessRecord {
    process_id: u32,
    parent_process_id: u32,
    started_at: Option<u64>,
    executable_name: String,
    executable_path: Option<String>,
    architecture: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct ProcessInstanceId {
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

trait ProcessInventory {
    fn snapshot(&mut self) -> Result<Vec<ProcessRecord>, PluginError>;
}

struct SystemProcessInventory;

impl ProcessInventory for SystemProcessInventory {
    fn snapshot(&mut self) -> Result<Vec<ProcessRecord>, PluginError> {
        enumerate_processes()
    }
}

#[derive(Clone, Debug)]
struct AuthorizedProcess {
    process: ProcessRecord,
    is_root: bool,
}

pub struct WindowsController {
    executable_names: BTreeSet<String>,
    executable_paths: BTreeSet<String>,
    descendant_executable_names: BTreeSet<String>,
    adapter_requirements: Vec<WireAdapterRequirement>,
    process_inventory: Box<dyn ProcessInventory>,
    admitted_instances: BTreeSet<ProcessInstanceId>,
    targets: BTreeMap<String, ProcessRecord>,
    runtime_libraries: BTreeMap<String, std::path::PathBuf>,
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

    fn with_process_inventory(process_inventory: Box<dyn ProcessInventory>) -> Self {
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

    fn authorized_processes(&mut self, processes: Vec<ProcessRecord>) -> Vec<AuthorizedProcess> {
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

fn has_authorized_ancestor(
    mut process_id: u32,
    processes: &BTreeMap<u32, (u32, Option<ProcessInstanceId>)>,
    anchors: &BTreeSet<ProcessInstanceId>,
) -> bool {
    let mut visited = BTreeSet::new();
    while process_id != 0 && visited.insert(process_id) {
        let Some((parent_process_id, instance)) = processes.get(&process_id) else {
            return false;
        };
        if instance.is_some_and(|instance| anchors.contains(&instance)) {
            return true;
        }
        process_id = *parent_process_id;
    }
    false
}

fn windows_executable(
    path: &Path,
    known_running: Option<bool>,
) -> Result<WindowsExecutable, PluginError> {
    let metadata =
        std::fs::metadata(path).map_err(|_| PluginError::new("invalid_executable_path"))?;
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|_| metadata.is_file())
        .filter(|value| {
            Path::new(value)
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
        })
        .ok_or_else(|| PluginError::new("invalid_executable_path"))?;
    if !path.is_absolute() {
        return Err(PluginError::new("invalid_executable_path"));
    }
    let architecture = std::fs::read(path)
        .ok()
        .and_then(|source| pe_architecture(&source))
        .ok_or_else(|| PluginError::new("invalid_executable_image"))?;
    let canonical =
        std::fs::canonicalize(path).map_err(|_| PluginError::new("invalid_executable_path"))?;
    let normalized = normalize_executable_path(&canonical.to_string_lossy());
    let running = known_running.unwrap_or_else(|| {
        enumerate_processes().is_ok_and(|processes| {
            processes.iter().any(|process| {
                process
                    .executable_path
                    .as_deref()
                    .is_some_and(|candidate| candidate == normalized)
            })
        })
    });

    Ok(WindowsExecutable {
        path: canonical,
        name: name.into(),
        architecture: architecture.into(),
        running,
    })
}

fn pe_architecture(source: &[u8]) -> Option<&'static str> {
    if source.get(0..2)? != b"MZ" {
        return None;
    }
    let pe_offset = u32::from_le_bytes(source.get(0x3c..0x40)?.try_into().ok()?) as usize;
    if source.get(pe_offset..pe_offset.checked_add(4)?)? != b"PE\0\0" {
        return None;
    }
    let machine_offset = pe_offset.checked_add(4)?;
    let machine = u16::from_le_bytes(
        source
            .get(machine_offset..machine_offset.checked_add(2)?)?
            .try_into()
            .ok()?,
    );
    match machine {
        0x8664 => Some("x86_64"),
        0x014c => Some("x86"),
        0xaa64 => Some("arm64"),
        _ => Some("unknown"),
    }
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
            parent_process_id: entry.th32ParentProcessID,
            started_at: process_started_at(entry.th32ProcessID),
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
fn process_started_at(process_id: u32) -> Option<u64> {
    use windows_sys::Win32::Foundation::{CloseHandle, FILETIME};
    use windows_sys::Win32::System::Threading::{
        GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, process_id) };
    if process.is_null() {
        return None;
    }
    let mut created = FILETIME::default();
    let mut exited = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    let queried =
        unsafe { GetProcessTimes(process, &mut created, &mut exited, &mut kernel, &mut user) } != 0;
    unsafe {
        CloseHandle(process);
    }
    queried.then_some(((created.dwHighDateTime as u64) << 32) | created.dwLowDateTime as u64)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    struct SyntheticProcessInventory {
        snapshots: VecDeque<Vec<ProcessRecord>>,
    }

    impl SyntheticProcessInventory {
        fn new(snapshots: impl IntoIterator<Item = Vec<ProcessRecord>>) -> Self {
            Self {
                snapshots: snapshots.into_iter().collect(),
            }
        }
    }

    impl ProcessInventory for SyntheticProcessInventory {
        fn snapshot(&mut self) -> Result<Vec<ProcessRecord>, PluginError> {
            Ok(self.snapshots.pop_front().unwrap_or_default())
        }
    }

    fn process(
        process_id: u32,
        parent_process_id: u32,
        started_at: Option<u64>,
        executable_name: &str,
    ) -> ProcessRecord {
        ProcessRecord {
            process_id,
            parent_process_id,
            started_at,
            executable_name: executable_name.into(),
            executable_path: None,
            architecture: "x86_64".into(),
        }
    }

    fn process_family_controller(
        snapshots: impl IntoIterator<Item = Vec<ProcessRecord>>,
    ) -> WindowsController {
        let mut controller = WindowsController::with_process_inventory(Box::new(
            SyntheticProcessInventory::new(snapshots),
        ));
        controller
            .configure(
                "org.example.process-family",
                &WireControllerConfiguration {
                    executable_names: vec!["Editor.exe".into()],
                    executable_paths: Vec::new(),
                    descendant_executable_names: vec!["Renderer.exe".into(), "Worker.exe".into()],
                    adapter_requirements: vec![WireAdapterRequirement {
                        adapter_id: "example.text".into(),
                        version_major: 1,
                        version_minor: 0,
                        version_patch: 0,
                        features: vec![WireFeature::TextReplace],
                    }],
                },
            )
            .expect("synthetic process family configuration");
        controller
    }

    #[test]
    fn process_family_inventory_keeps_stable_tokens_and_surviving_members_after_root_exit() {
        let initial = vec![
            process(100, 1, Some(1_000), "Editor.exe"),
            process(110, 100, Some(1_100), "Renderer.exe"),
            process(120, 110, Some(1_200), "Worker.exe"),
            process(130, 999, Some(1_300), "Renderer.exe"),
            process(140, 100, None, "Worker.exe"),
        ];
        let reordered = vec![initial[2].clone(), initial[0].clone(), initial[1].clone()];
        let after_root_exit = vec![
            initial[2].clone(),
            initial[1].clone(),
            process(121, 110, Some(1_210), "Worker.exe"),
        ];
        let after_member_exit = vec![after_root_exit[0].clone(), after_root_exit[2].clone()];
        let mut controller =
            process_family_controller([initial, reordered, after_root_exit, after_member_exit]);

        let first = controller.inventory().expect("initial family inventory");
        assert_eq!(
            first
                .targets
                .iter()
                .map(|target| target.token.as_str())
                .collect::<Vec<_>>(),
            vec!["target:1", "target:2", "target:3"]
        );
        assert_eq!(first.targets[0].display_name, "Editor · 运行中");
        assert_eq!(first.targets[1].display_name, "Renderer · 运行中");
        assert_eq!(first.targets[2].display_name, "Worker · 运行中");

        let second = controller.inventory().expect("reordered family inventory");
        assert_eq!(
            second
                .targets
                .iter()
                .map(|target| target.token.as_str())
                .collect::<Vec<_>>(),
            vec!["target:1", "target:2", "target:3"]
        );

        let third = controller
            .inventory()
            .expect("surviving family member inventory");
        assert_eq!(
            third
                .targets
                .iter()
                .map(|target| target.token.as_str())
                .collect::<Vec<_>>(),
            vec!["target:2", "target:3", "target:4"]
        );
        controller
            .runtime_libraries
            .insert("target:2".into(), "synthetic-runtime.dll".into());

        let fourth = controller.inventory().expect("partial exit inventory");
        assert_eq!(
            fourth
                .targets
                .iter()
                .map(|target| target.token.as_str())
                .collect::<Vec<_>>(),
            vec!["target:3", "target:4"]
        );
        assert!(!controller.runtime_libraries.contains_key("target:2"));
        assert!(controller
            .prepare("target:2", &[WireFeature::TextReplace])
            .is_err());
        assert!(controller
            .prepare("target:3", &[WireFeature::TextReplace])
            .is_ok());
    }

    #[test]
    fn process_family_inventory_never_reuses_a_token_after_process_id_reuse() {
        let mut controller = process_family_controller([
            vec![process(100, 1, Some(1_000), "Editor.exe")],
            Vec::new(),
            vec![process(100, 1, Some(2_000), "Editor.exe")],
        ]);

        let first = controller.inventory().expect("first process instance");
        assert_eq!(first.targets[0].token, "target:1");
        assert!(controller
            .inventory()
            .expect("process exit inventory")
            .targets
            .is_empty());
        let replacement = controller
            .inventory()
            .expect("replacement process instance");
        assert_eq!(replacement.targets[0].token, "target:2");
    }

    #[test]
    fn same_executable_descendant_does_not_outrank_its_top_level_root() {
        let mut controller = process_family_controller([vec![
            process(100, 200, Some(1_100), "Editor.exe"),
            process(200, 1, Some(1_000), "Editor.exe"),
        ]]);

        let inventory = controller.inventory().expect("same-executable family");

        assert_eq!(inventory.targets.len(), 2);
        assert_eq!(
            controller.targets[&inventory.targets[0].token].process_id,
            200
        );
        assert_eq!(
            controller.targets[&inventory.targets[1].token].process_id,
            100
        );
    }

    #[test]
    fn independent_same_executable_instances_remain_top_level_roots() {
        let mut controller = process_family_controller([Vec::new()]);

        let authorized = controller.authorized_processes(vec![
            process(100, 1, Some(1_000), "Editor.exe"),
            process(200, 1, Some(2_000), "Editor.exe"),
        ]);

        assert_eq!(authorized.len(), 2);
        assert!(authorized.iter().all(|process| process.is_root));
    }

    #[test]
    fn pe_header_reports_only_the_windows_target_architectures_we_can_screen() {
        fn image(machine: u16) -> Vec<u8> {
            let mut bytes = vec![0_u8; 256];
            bytes[0..2].copy_from_slice(b"MZ");
            bytes[0x3c..0x40].copy_from_slice(&0x80_u32.to_le_bytes());
            bytes[0x80..0x84].copy_from_slice(b"PE\0\0");
            bytes[0x84..0x86].copy_from_slice(&machine.to_le_bytes());
            bytes
        }

        assert_eq!(pe_architecture(&image(0x8664)), Some("x86_64"));
        assert_eq!(pe_architecture(&image(0x014c)), Some("x86"));
        assert_eq!(pe_architecture(&image(0xaa64)), Some("arm64"));
        assert_eq!(pe_architecture(b"not a PE image"), None);
    }
}

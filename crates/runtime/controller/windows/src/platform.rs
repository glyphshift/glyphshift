use crate::controller::{ProcessInstanceId, ProcessRecord};
use crate::WindowsExecutable;
use glyphshift_controller_sdk::PluginError;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::path::Path;

pub(super) fn file_sha256(path: &Path) -> Result<[u8; 32], ()> {
    let mut file = File::open(path).map_err(|_| ())?;
    let mut digest = Sha256::new();
    std::io::copy(&mut file, &mut digest).map_err(|_| ())?;
    Ok(digest.finalize().into())
}

pub(super) fn validate_executable_name(name: &str) -> Result<String, PluginError> {
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

pub(super) fn validate_executable_path(path: &str) -> Result<String, PluginError> {
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
    let canonical =
        std::fs::canonicalize(path).map_err(|_| PluginError::new("invalid_executable_path"))?;
    Ok(normalize_executable_path(&canonical.to_string_lossy()))
}

fn normalize_executable_path(path: &str) -> String {
    path.strip_prefix(r"\\?\")
        .unwrap_or(path)
        .replace('/', "\\")
        .to_lowercase()
}

pub(super) fn has_authorized_ancestor(
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

pub(super) fn windows_executable(
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

pub(super) fn pe_architecture(source: &[u8]) -> Option<&'static str> {
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
pub(super) fn enumerate_processes() -> Result<Vec<ProcessRecord>, PluginError> {
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
pub(super) fn process_started_at(process_id: u32) -> Option<u64> {
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
pub(super) fn process_executable_path(process_id: u32) -> Option<String> {
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
    let queried = String::from_utf16_lossy(&buffer[..length as usize]);
    let resolved = std::fs::canonicalize(Path::new(queried.as_str()))
        .unwrap_or_else(|_| Path::new(queried.as_str()).to_path_buf());
    Some(normalize_executable_path(&resolved.to_string_lossy()))
}

#[cfg(windows)]
pub(super) fn process_architecture(process_id: u32) -> String {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::SystemInformation::{
        IMAGE_FILE_MACHINE_AMD64, IMAGE_FILE_MACHINE_I386, IMAGE_FILE_MACHINE_UNKNOWN,
    };
    use windows_sys::Win32::System::Threading::{
        IsWow64Process2, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, process_id) };
    if process.is_null() {
        return "unknown".into();
    }
    let mut process_machine = IMAGE_FILE_MACHINE_UNKNOWN;
    let mut native_machine = IMAGE_FILE_MACHINE_UNKNOWN;
    let queried =
        unsafe { IsWow64Process2(process, &mut process_machine, &mut native_machine) } != 0;
    unsafe {
        CloseHandle(process);
    }
    if !queried {
        return "unknown".into();
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
pub(super) fn enumerate_processes() -> Result<Vec<ProcessRecord>, PluginError> {
    Err(PluginError::new("unsupported_operating_system"))
}

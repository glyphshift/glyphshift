use glyphshift_capture::CaptureObservationBatch;
use glyphshift_target_runtime_contract::{
    RuntimeCommandV1, RuntimeDiagnosticsControl, RuntimeDiagnosticsQueryV1,
    RuntimeObservationQueryV1, RuntimeTraceBatch, MAX_RUNTIME_OBSERVATION_BYTES,
    MAX_RUNTIME_TRACE_BYTES, STATUS_TARGET_RUNTIME_OK,
};
use std::ffi::c_void;
use std::mem::size_of;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::ptr::null_mut;
use windows::core::{s, w, PCWSTR};
use windows::Win32::Foundation::{CloseHandle, FreeLibrary, HANDLE, WAIT_OBJECT_0};
use windows::Win32::System::Diagnostics::Debug::{ReadProcessMemory, WriteProcessMemory};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Module32FirstW, Module32NextW, MODULEENTRY32W, TH32CS_SNAPMODULE,
    TH32CS_SNAPMODULE32,
};
use windows::Win32::System::LibraryLoader::{
    GetModuleHandleW, GetProcAddress, LoadLibraryExW, DONT_RESOLVE_DLL_REFERENCES,
};
use windows::Win32::System::Memory::{
    VirtualAllocEx, VirtualFreeEx, MEM_COMMIT, MEM_RELEASE, MEM_RESERVE, PAGE_READWRITE,
};
use windows::Win32::System::Threading::{
    CreateRemoteThread, GetExitCodeThread, OpenProcess, WaitForSingleObject,
    LPTHREAD_START_ROUTINE, PROCESS_CREATE_THREAD, PROCESS_QUERY_INFORMATION, PROCESS_VM_OPERATION,
    PROCESS_VM_READ, PROCESS_VM_WRITE,
};

const REMOTE_TIMEOUT_MS: u32 = 15_000;

#[derive(Clone, Copy, Debug)]
pub enum RemoteError {
    ProcessUnavailable,
    AllocationFailed,
    WriteFailed,
    ReadFailed,
    ModuleUnavailable,
    ExportUnavailable,
    ThreadFailed,
    Timeout,
    RemoteRejected(u32),
}

impl RemoteError {
    pub fn code(self) -> String {
        match self {
            Self::ProcessUnavailable => "process_unavailable".into(),
            Self::AllocationFailed => "remote_allocation_failed".into(),
            Self::WriteFailed => "remote_write_failed".into(),
            Self::ReadFailed => "remote_read_failed".into(),
            Self::ModuleUnavailable => "runtime_module_unavailable".into(),
            Self::ExportUnavailable => "runtime_export_unavailable".into(),
            Self::ThreadFailed => "remote_thread_failed".into(),
            Self::Timeout => "remote_thread_timeout".into(),
            Self::RemoteRejected(status) => format!("target_runtime_rejected_{status}"),
        }
    }
}

struct ProcessHandle(HANDLE);

impl ProcessHandle {
    fn open(process_id: u32) -> Result<Self, RemoteError> {
        let access = PROCESS_CREATE_THREAD
            | PROCESS_QUERY_INFORMATION
            | PROCESS_VM_OPERATION
            | PROCESS_VM_WRITE
            | PROCESS_VM_READ;
        unsafe { OpenProcess(access, false, process_id) }
            .map(Self)
            .map_err(|_| RemoteError::ProcessUnavailable)
    }
}

impl Drop for ProcessHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

struct RemoteAllocation {
    process: HANDLE,
    address: *mut c_void,
}

impl RemoteAllocation {
    fn allocate(process: HANDLE, len: usize) -> Result<Self, RemoteError> {
        let address = unsafe {
            VirtualAllocEx(
                process,
                None,
                len.max(1),
                MEM_COMMIT | MEM_RESERVE,
                PAGE_READWRITE,
            )
        };
        if address.is_null() {
            return Err(RemoteError::AllocationFailed);
        }
        Ok(Self { process, address })
    }

    fn write(process: HANDLE, bytes: &[u8]) -> Result<Self, RemoteError> {
        let allocation = Self::allocate(process, bytes.len())?;
        if unsafe {
            WriteProcessMemory(
                process,
                allocation.address,
                bytes.as_ptr().cast(),
                bytes.len(),
                None,
            )
        }
        .is_err()
        {
            return Err(RemoteError::WriteFailed);
        }
        Ok(allocation)
    }

    fn read(&self, output: &mut [u8]) -> Result<(), RemoteError> {
        unsafe {
            ReadProcessMemory(
                self.process,
                self.address,
                output.as_mut_ptr().cast(),
                output.len(),
                None,
            )
        }
        .map_err(|_| RemoteError::ReadFailed)
    }
}

impl Drop for RemoteAllocation {
    fn drop(&mut self) {
        unsafe {
            let _ = VirtualFreeEx(self.process, self.address, 0, MEM_RELEASE);
        }
    }
}

pub fn activate(
    process_id: u32,
    runtime_library: &Path,
    deployment_json: &str,
) -> Result<(), RemoteError> {
    let process = ProcessHandle::open(process_id)?;
    inject_library(process_id, process.0, runtime_library)?;
    invoke_json_export(
        process_id,
        process.0,
        runtime_library,
        "glyphshift_runtime_activate_v1",
        deployment_json,
    )
}

pub fn update(
    process_id: u32,
    runtime_library: &Path,
    publication_json: &str,
) -> Result<(), RemoteError> {
    let process = ProcessHandle::open(process_id)?;
    invoke_json_export(
        process_id,
        process.0,
        runtime_library,
        "glyphshift_runtime_update_v1",
        publication_json,
    )
}

pub fn control_capture(
    process_id: u32,
    runtime_library: &Path,
    paused: bool,
) -> Result<(), RemoteError> {
    let command = if paused {
        r#"{"paused":true}"#
    } else {
        r#"{"paused":false}"#
    };
    let process = ProcessHandle::open(process_id)?;
    invoke_json_export(
        process_id,
        process.0,
        runtime_library,
        "glyphshift_runtime_capture_control_v1",
        command,
    )
}

pub fn control_diagnostics(
    process_id: u32,
    runtime_library: &Path,
    enabled: bool,
) -> Result<(), RemoteError> {
    let command = RuntimeDiagnosticsControl::new(enabled)
        .encode_json()
        .map_err(|_| RemoteError::WriteFailed)?;
    let process = ProcessHandle::open(process_id)?;
    invoke_json_export(
        process_id,
        process.0,
        runtime_library,
        "glyphshift_runtime_diagnostics_control_v1",
        &command,
    )
}

pub fn query_diagnostics(
    process_id: u32,
    runtime_library: &Path,
) -> Result<RuntimeTraceBatch, RemoteError> {
    let json = query_json_export(
        process_id,
        runtime_library,
        "glyphshift_runtime_diagnostics_query_v1",
        MAX_RUNTIME_TRACE_BYTES,
        size_of::<RuntimeDiagnosticsQueryV1>(),
    )?;
    RuntimeTraceBatch::decode_json(&json).map_err(|_| RemoteError::ReadFailed)
}

pub fn query_observations(
    process_id: u32,
    runtime_library: &Path,
) -> Result<CaptureObservationBatch, RemoteError> {
    let json = query_json_export(
        process_id,
        runtime_library,
        "glyphshift_runtime_observation_query_v1",
        MAX_RUNTIME_OBSERVATION_BYTES,
        size_of::<RuntimeObservationQueryV1>(),
    )?;
    CaptureObservationBatch::decode_json(&json).map_err(|_| RemoteError::ReadFailed)
}

#[repr(C)]
struct RemoteJsonQueryV1 {
    struct_size: u32,
    output: *mut u8,
    output_capacity: u32,
    output_len: u32,
}

fn query_json_export(
    process_id: u32,
    runtime_library: &Path,
    export: &str,
    max_output_bytes: usize,
    expected_struct_size: usize,
) -> Result<String, RemoteError> {
    if size_of::<RemoteJsonQueryV1>() != expected_struct_size {
        return Err(RemoteError::ReadFailed);
    }
    let process = ProcessHandle::open(process_id)?;
    let remote_output = RemoteAllocation::allocate(process.0, max_output_bytes)?;
    let query = RemoteJsonQueryV1 {
        struct_size: size_of::<RemoteJsonQueryV1>() as u32,
        output: remote_output.address.cast(),
        output_capacity: max_output_bytes as u32,
        output_len: 0,
    };
    let query_bytes = unsafe {
        std::slice::from_raw_parts(
            (&query as *const RemoteJsonQueryV1).cast::<u8>(),
            size_of::<RemoteJsonQueryV1>(),
        )
    };
    let remote_query = RemoteAllocation::write(process.0, query_bytes)?;
    let function = remote_export(process_id, runtime_library, export)?;
    let status = run_remote_thread(process.0, function, Some(remote_query.address.cast_const()))?;
    if status != STATUS_TARGET_RUNTIME_OK {
        return Err(RemoteError::RemoteRejected(status));
    }
    let mut returned_query = vec![0_u8; size_of::<RemoteJsonQueryV1>()];
    remote_query.read(&mut returned_query)?;
    let returned_query =
        unsafe { std::ptr::read_unaligned(returned_query.as_ptr().cast::<RemoteJsonQueryV1>()) };
    if returned_query.output_len as usize > max_output_bytes {
        return Err(RemoteError::ReadFailed);
    }
    let mut output = vec![0_u8; returned_query.output_len as usize];
    remote_output.read(&mut output)?;
    String::from_utf8(output).map_err(|_| RemoteError::ReadFailed)
}

pub fn deactivate(process_id: u32, runtime_library: &Path) -> Result<(), RemoteError> {
    let process = ProcessHandle::open(process_id)?;
    let function = remote_export(
        process_id,
        runtime_library,
        "glyphshift_runtime_deactivate_v1",
    )?;
    let status = run_remote_thread(process.0, function, None)?;
    (status == STATUS_TARGET_RUNTIME_OK)
        .then_some(())
        .ok_or(RemoteError::RemoteRejected(status))
}

fn inject_library(process_id: u32, process: HANDLE, library: &Path) -> Result<(), RemoteError> {
    if remote_module_base(process_id, library).is_ok() {
        return Ok(());
    }
    let wide = library
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let bytes = unsafe {
        std::slice::from_raw_parts(wide.as_ptr().cast::<u8>(), wide.len() * size_of::<u16>())
    };
    let remote_path = RemoteAllocation::write(process, bytes)?;
    let load_library = remote_system_export(process_id, "kernel32.dll", "LoadLibraryW")?;
    let result = run_remote_thread(
        process,
        load_library,
        Some(remote_path.address.cast_const()),
    )?;
    if result == 0 || remote_module_base(process_id, library).is_err() {
        return Err(RemoteError::ModuleUnavailable);
    }
    Ok(())
}

fn invoke_json_export(
    process_id: u32,
    process: HANDLE,
    runtime_library: &Path,
    export: &str,
    json: &str,
) -> Result<(), RemoteError> {
    let remote_json = RemoteAllocation::write(process, json.as_bytes())?;
    let command = RuntimeCommandV1 {
        struct_size: size_of::<RuntimeCommandV1>() as u32,
        json: remote_json.address.cast(),
        json_len: json.len() as u32,
    };
    let command_bytes = unsafe {
        std::slice::from_raw_parts(
            (&command as *const RuntimeCommandV1).cast::<u8>(),
            size_of::<RuntimeCommandV1>(),
        )
    };
    let remote_command = RemoteAllocation::write(process, command_bytes)?;
    let function = remote_export(process_id, runtime_library, export)?;
    let status = run_remote_thread(process, function, Some(remote_command.address.cast_const()))?;
    (status == STATUS_TARGET_RUNTIME_OK)
        .then_some(())
        .ok_or(RemoteError::RemoteRejected(status))
}

fn run_remote_thread(
    process: HANDLE,
    address: usize,
    parameter: Option<*const c_void>,
) -> Result<u32, RemoteError> {
    let start: LPTHREAD_START_ROUTINE = Some(unsafe {
        std::mem::transmute::<usize, unsafe extern "system" fn(*mut c_void) -> u32>(address)
    });
    let thread = unsafe {
        CreateRemoteThread(process, None, 0, start, parameter, 0, None)
            .map_err(|_| RemoteError::ThreadFailed)?
    };
    let wait = unsafe { WaitForSingleObject(thread, REMOTE_TIMEOUT_MS) };
    if wait != WAIT_OBJECT_0 {
        unsafe {
            let _ = CloseHandle(thread);
        }
        return Err(RemoteError::Timeout);
    }
    let mut exit_code = 0_u32;
    let result = unsafe { GetExitCodeThread(thread, &mut exit_code) }
        .map(|()| exit_code)
        .map_err(|_| RemoteError::ThreadFailed);
    unsafe {
        let _ = CloseHandle(thread);
    }
    result
}

fn remote_export(process_id: u32, library: &Path, export: &str) -> Result<usize, RemoteError> {
    let wide = library
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let local = unsafe {
        LoadLibraryExW(
            PCWSTR(wide.as_ptr()),
            HANDLE(null_mut()),
            DONT_RESOLVE_DLL_REFERENCES,
        )
        .map_err(|_| RemoteError::ModuleUnavailable)?
    };
    let name = std::ffi::CString::new(export).map_err(|_| RemoteError::ExportUnavailable)?;
    let address = unsafe { GetProcAddress(local, windows::core::PCSTR(name.as_ptr().cast())) }
        .ok_or(RemoteError::ExportUnavailable)? as usize;
    let offset = address
        .checked_sub(local.0 as usize)
        .ok_or(RemoteError::ExportUnavailable)?;
    unsafe {
        let _ = FreeLibrary(local);
    }
    Ok(remote_module_base(process_id, library)? + offset)
}

fn remote_system_export(
    process_id: u32,
    module_name: &str,
    export: &str,
) -> Result<usize, RemoteError> {
    let local = unsafe { GetModuleHandleW(w!("kernel32.dll")) }
        .map_err(|_| RemoteError::ModuleUnavailable)?;
    let address = unsafe {
        match export {
            "LoadLibraryW" => GetProcAddress(local, s!("LoadLibraryW")),
            _ => None,
        }
    }
    .ok_or(RemoteError::ExportUnavailable)? as usize;
    let offset = address
        .checked_sub(local.0 as usize)
        .ok_or(RemoteError::ExportUnavailable)?;
    Ok(remote_module_base_by_name(process_id, module_name)? + offset)
}

fn remote_module_base(process_id: u32, library: &Path) -> Result<usize, RemoteError> {
    let name = library
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or(RemoteError::ModuleUnavailable)?;
    remote_module_base_by_name(process_id, name)
}

fn remote_module_base_by_name(process_id: u32, name: &str) -> Result<usize, RemoteError> {
    let snapshot = unsafe {
        CreateToolhelp32Snapshot(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, process_id)
            .map_err(|_| RemoteError::ModuleUnavailable)?
    };
    let mut entry = MODULEENTRY32W {
        dwSize: size_of::<MODULEENTRY32W>() as u32,
        ..Default::default()
    };
    let mut found = None;
    if unsafe { Module32FirstW(snapshot, &mut entry) }.is_ok() {
        loop {
            let length = entry
                .szModule
                .iter()
                .position(|unit| *unit == 0)
                .unwrap_or(entry.szModule.len());
            let module = String::from_utf16_lossy(&entry.szModule[..length]);
            if module.eq_ignore_ascii_case(name) {
                found = Some(entry.modBaseAddr as usize);
                break;
            }
            if unsafe { Module32NextW(snapshot, &mut entry) }.is_err() {
                break;
            }
        }
    }
    unsafe {
        let _ = CloseHandle(snapshot);
    }
    found.ok_or(RemoteError::ModuleUnavailable)
}

use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};
use windows_sys::Win32::Foundation::{CloseHandle, FILETIME};
use windows_sys::Win32::Security::{
    GetSidSubAuthority, GetSidSubAuthorityCount, GetTokenInformation, TokenIntegrityLevel,
    TOKEN_MANDATORY_LABEL, TOKEN_QUERY,
};
use windows_sys::Win32::System::Threading::{
    GetProcessTimes, OpenProcess, OpenProcessToken, PROCESS_QUERY_LIMITED_INFORMATION,
};

const TARGET_GRANT_PLATFORM: &str = "windows-process-v1";

#[derive(Clone, Copy)]
pub(crate) struct ProcessInstance {
    pub(crate) process_id: u32,
    pub(crate) started_at: u64,
}

pub(crate) struct ComApartment;

impl ComApartment {
    pub(crate) fn enter() -> Result<Self, WindowsTargetError> {
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }
            .ok()
            .map_err(|_| WindowsTargetError::ComUnavailable)?;
        Ok(Self)
    }
}

impl Drop for ComApartment {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WindowsTargetError {
    ActivationRejected,
    InvalidGrant,
    TargetChanged,
    PermissionDenied,
    ComUnavailable,
}

pub(crate) fn authorize_process_target(
    adapter_id: &str,
    expected_adapter_id: &str,
    platform: &str,
    payload: &str,
) -> Result<ProcessInstance, WindowsTargetError> {
    if adapter_id != expected_adapter_id || platform != TARGET_GRANT_PLATFORM {
        return Err(WindowsTargetError::ActivationRejected);
    }
    let (process_id, started_at) = payload
        .split_once(':')
        .ok_or(WindowsTargetError::InvalidGrant)?;
    let process_id = process_id
        .parse::<u32>()
        .ok()
        .filter(|value| *value > 0)
        .ok_or(WindowsTargetError::InvalidGrant)?;
    let started_at = started_at
        .parse::<u64>()
        .ok()
        .filter(|value| *value > 0)
        .ok_or(WindowsTargetError::InvalidGrant)?;
    if process_started_at(process_id) != Some(started_at) {
        return Err(WindowsTargetError::TargetChanged);
    }
    let worker_integrity =
        process_integrity_rid(std::process::id()).ok_or(WindowsTargetError::PermissionDenied)?;
    let target_integrity =
        process_integrity_rid(process_id).ok_or(WindowsTargetError::PermissionDenied)?;
    if target_integrity > worker_integrity {
        return Err(WindowsTargetError::PermissionDenied);
    }
    Ok(ProcessInstance {
        process_id,
        started_at,
    })
}

pub(crate) fn process_started_at(process_id: u32) -> Option<u64> {
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
    unsafe { CloseHandle(process) };
    queried.then_some(((created.dwHighDateTime as u64) << 32) | created.dwLowDateTime as u64)
}

fn process_integrity_rid(process_id: u32) -> Option<u32> {
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, process_id) };
    if process.is_null() {
        return None;
    }
    let mut token = std::ptr::null_mut();
    if unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut token) } == 0 {
        unsafe { CloseHandle(process) };
        return None;
    }
    let mut required = 0;
    unsafe {
        GetTokenInformation(
            token,
            TokenIntegrityLevel,
            std::ptr::null_mut(),
            0,
            &mut required,
        );
    }
    let mut buffer = vec![0_u8; required as usize];
    let queried = required > 0
        && unsafe {
            GetTokenInformation(
                token,
                TokenIntegrityLevel,
                buffer.as_mut_ptr().cast(),
                required,
                &mut required,
            )
        } != 0;
    let integrity = queried.then(|| unsafe {
        let label = &*buffer.as_ptr().cast::<TOKEN_MANDATORY_LABEL>();
        let count = *GetSidSubAuthorityCount(label.Label.Sid) as u32;
        *GetSidSubAuthority(label.Label.Sid, count.saturating_sub(1))
    });
    unsafe {
        CloseHandle(token);
        CloseHandle(process);
    }
    integrity
}

//! Windows process-instance grant validation shared by isolated and one-shot workers.
//!
//! A grant binds an expected Adapter to one PID plus its creation timestamp. Validation also
//! prevents a worker from reading a target at a higher Windows integrity level.

pub const WINDOWS_PROCESS_GRANT_PLATFORM: &str = "windows-process-v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AuthorizedProcess {
    process_id: u32,
    started_at: u64,
}

impl AuthorizedProcess {
    #[must_use]
    pub const fn process_id(self) -> u32 {
        self.process_id
    }

    #[must_use]
    pub const fn started_at(self) -> u64 {
        self.started_at
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProcessGrantError {
    ActivationRejected,
    InvalidGrant,
    TargetChanged,
    PermissionDenied,
}

#[cfg(windows)]
mod platform {
    use super::*;
    use windows_sys::Win32::Foundation::{CloseHandle, FILETIME};
    use windows_sys::Win32::Security::{
        GetSidSubAuthority, GetSidSubAuthorityCount, GetTokenInformation, TokenIntegrityLevel,
        TOKEN_MANDATORY_LABEL, TOKEN_QUERY,
    };
    use windows_sys::Win32::System::Threading::{
        GetProcessTimes, OpenProcess, OpenProcessToken, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    pub fn authorize_process_target(
        adapter_id: &str,
        expected_adapter_id: &str,
        platform: &str,
        payload: &str,
    ) -> Result<AuthorizedProcess, ProcessGrantError> {
        if adapter_id != expected_adapter_id || platform != WINDOWS_PROCESS_GRANT_PLATFORM {
            return Err(ProcessGrantError::ActivationRejected);
        }
        let (process_id, started_at) = parse_payload(payload)?;
        if process_started_at(process_id) != Some(started_at) {
            return Err(ProcessGrantError::TargetChanged);
        }
        let worker_integrity =
            process_integrity_rid(std::process::id()).ok_or(ProcessGrantError::PermissionDenied)?;
        let target_integrity =
            process_integrity_rid(process_id).ok_or(ProcessGrantError::PermissionDenied)?;
        if target_integrity > worker_integrity {
            return Err(ProcessGrantError::PermissionDenied);
        }
        Ok(AuthorizedProcess {
            process_id,
            started_at,
        })
    }

    fn parse_payload(payload: &str) -> Result<(u32, u64), ProcessGrantError> {
        let (process_id, started_at) = payload
            .split_once(':')
            .ok_or(ProcessGrantError::InvalidGrant)?;
        let process_id = process_id
            .parse::<u32>()
            .ok()
            .filter(|value| *value > 0)
            .ok_or(ProcessGrantError::InvalidGrant)?;
        let started_at = started_at
            .parse::<u64>()
            .ok()
            .filter(|value| *value > 0)
            .ok_or(ProcessGrantError::InvalidGrant)?;
        Ok((process_id, started_at))
    }

    pub fn process_started_at(process_id: u32) -> Option<u64> {
        let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, process_id) };
        if process.is_null() {
            return None;
        }
        let mut created = FILETIME::default();
        let mut exited = FILETIME::default();
        let mut kernel = FILETIME::default();
        let mut user = FILETIME::default();
        let queried =
            unsafe { GetProcessTimes(process, &mut created, &mut exited, &mut kernel, &mut user) }
                != 0;
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

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn current_process_grant_binds_adapter_platform_and_start_time() {
            let process_id = std::process::id();
            let started_at = process_started_at(process_id).expect("current process start time");
            let payload = format!("{process_id}:{started_at}");

            let authorized = authorize_process_target(
                "windows.fixture.acquire",
                "windows.fixture.acquire",
                WINDOWS_PROCESS_GRANT_PLATFORM,
                &payload,
            )
            .expect("current process grant");

            assert_eq!(authorized.process_id(), process_id);
            assert_eq!(authorized.started_at(), started_at);
            assert_eq!(
                authorize_process_target(
                    "windows.other.acquire",
                    "windows.fixture.acquire",
                    WINDOWS_PROCESS_GRANT_PLATFORM,
                    &payload,
                ),
                Err(ProcessGrantError::ActivationRejected)
            );
            assert_eq!(
                authorize_process_target(
                    "windows.fixture.acquire",
                    "windows.fixture.acquire",
                    WINDOWS_PROCESS_GRANT_PLATFORM,
                    &format!("{process_id}:{}", started_at.saturating_add(1)),
                ),
                Err(ProcessGrantError::TargetChanged)
            );
        }

        #[test]
        fn malformed_process_payloads_fail_before_process_access() {
            for payload in ["", "1", "0:1", "1:0", "one:two", "1:2:3"] {
                assert_eq!(parse_payload(payload), Err(ProcessGrantError::InvalidGrant));
            }
        }
    }
}

#[cfg(windows)]
pub use platform::{authorize_process_target, process_started_at};

#[cfg(not(windows))]
pub fn authorize_process_target(
    _adapter_id: &str,
    _expected_adapter_id: &str,
    _platform: &str,
    _payload: &str,
) -> Result<AuthorizedProcess, ProcessGrantError> {
    Err(ProcessGrantError::PermissionDenied)
}

#[cfg(not(windows))]
#[must_use]
pub const fn process_started_at(_process_id: u32) -> Option<u64> {
    None
}

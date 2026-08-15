#[cfg(windows)]
use crate::platform::process_executable_path;
use crate::platform::windows_executable;
use glyphshift_controller_sdk::PluginError;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WindowsExecutable {
    pub(super) path: PathBuf,
    pub(super) name: Box<str>,
    pub(super) architecture: Box<str>,
    pub(super) running: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WindowsForegroundPoint {
    executable: WindowsExecutable,
    x: i32,
    y: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowsElevationError {
    QueryFailed,
    InvalidExecutable,
    LaunchFailed,
    UnsupportedOperatingSystem,
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

impl WindowsForegroundPoint {
    #[must_use]
    pub const fn executable(&self) -> &WindowsExecutable {
        &self.executable
    }

    #[must_use]
    pub const fn x(&self) -> i32 {
        self.x
    }

    #[must_use]
    pub const fn y(&self) -> i32 {
        self.y
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

fn normalize_running_executables(
    mut executables: Vec<WindowsExecutable>,
) -> Vec<WindowsExecutable> {
    executables.sort_by(|left, right| {
        left.name
            .to_lowercase()
            .cmp(&right.name.to_lowercase())
            .then_with(|| {
                left.path
                    .to_string_lossy()
                    .to_lowercase()
                    .cmp(&right.path.to_string_lossy().to_lowercase())
            })
    });
    executables.dedup_by(|left, right| {
        left.path
            .to_string_lossy()
            .eq_ignore_ascii_case(&right.path.to_string_lossy())
    });
    executables
}

#[cfg(windows)]
pub fn running_windows_executables() -> Result<Vec<WindowsExecutable>, PluginError> {
    use std::collections::BTreeSet;
    use windows_sys::Win32::Foundation::{HWND, LPARAM};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowTextLengthW, GetWindowThreadProcessId, IsWindowVisible,
    };

    unsafe extern "system" fn collect_visible_window_process(
        window: HWND,
        parameter: LPARAM,
    ) -> i32 {
        let process_ids = &mut *(parameter as *mut BTreeSet<u32>);
        if IsWindowVisible(window) == 0 || GetWindowTextLengthW(window) == 0 {
            return 1;
        }
        let mut process_id = 0_u32;
        GetWindowThreadProcessId(window, &mut process_id);
        if process_id != 0 && process_id != std::process::id() {
            process_ids.insert(process_id);
        }
        1
    }

    let mut process_ids = BTreeSet::new();
    let enumerated = unsafe {
        EnumWindows(
            Some(collect_visible_window_process),
            (&mut process_ids as *mut BTreeSet<u32>) as LPARAM,
        )
    };
    if enumerated == 0 {
        return Err(PluginError::new("visible_window_inventory_unavailable"));
    }

    let executables = process_ids
        .into_iter()
        .filter_map(process_executable_path)
        .filter_map(|path| windows_executable(Path::new(&path), Some(true)).ok())
        .collect();
    Ok(normalize_running_executables(executables))
}

#[cfg(not(windows))]
pub fn running_windows_executables() -> Result<Vec<WindowsExecutable>, PluginError> {
    Err(PluginError::new("unsupported_operating_system"))
}

#[cfg(windows)]
pub fn foreground_windows_point() -> Result<WindowsForegroundPoint, PluginError> {
    use windows_sys::Win32::Foundation::POINT;
    use windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos;

    let executable = foreground_windows_executable()?;
    let mut point = POINT::default();
    if unsafe { GetCursorPos(&mut point) } == 0 {
        return Err(PluginError::new("cursor_position_unavailable"));
    }
    Ok(WindowsForegroundPoint {
        executable,
        x: point.x,
        y: point.y,
    })
}

#[cfg(not(windows))]
pub fn foreground_windows_executable() -> Result<WindowsExecutable, PluginError> {
    Err(PluginError::new("unsupported_operating_system"))
}

#[cfg(not(windows))]
pub fn foreground_windows_point() -> Result<WindowsForegroundPoint, PluginError> {
    Err(PluginError::new("unsupported_operating_system"))
}

#[cfg(windows)]
pub fn current_process_is_elevated() -> Result<bool, WindowsElevationError> {
    use std::mem::size_of;
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::Security::{
        GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY,
    };
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    let mut token = std::ptr::null_mut();
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return Err(WindowsElevationError::QueryFailed);
    }
    let mut elevation = TOKEN_ELEVATION::default();
    let mut returned = 0_u32;
    let queried = unsafe {
        GetTokenInformation(
            token,
            TokenElevation,
            (&mut elevation as *mut TOKEN_ELEVATION).cast(),
            size_of::<TOKEN_ELEVATION>() as u32,
            &mut returned,
        )
    } != 0;
    unsafe {
        CloseHandle(token);
    }
    queried
        .then_some(elevation.TokenIsElevated != 0)
        .ok_or(WindowsElevationError::QueryFailed)
}

#[cfg(not(windows))]
pub const fn current_process_is_elevated() -> Result<bool, WindowsElevationError> {
    Err(WindowsElevationError::UnsupportedOperatingSystem)
}

#[cfg(windows)]
pub(super) const fn elevated_launch_mask() -> u32 {
    use windows_sys::Win32::UI::Shell::{SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS};

    SEE_MASK_NOASYNC | SEE_MASK_NOCLOSEPROCESS
}

#[cfg(windows)]
pub(super) fn elevated_launch_parameters(arguments: &[OsString]) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;

    let mut parameters = Vec::new();
    for (index, argument) in arguments.iter().enumerate() {
        if index > 0 {
            parameters.push(u16::from(b' '));
        }
        parameters.push(u16::from(b'"'));
        let mut backslashes = 0;
        for unit in argument.encode_wide() {
            match unit {
                value if value == u16::from(b'\\') => backslashes += 1,
                value if value == u16::from(b'"') => {
                    parameters.extend(std::iter::repeat_n(u16::from(b'\\'), backslashes * 2 + 1));
                    parameters.push(value);
                    backslashes = 0;
                }
                value => {
                    parameters.extend(std::iter::repeat_n(u16::from(b'\\'), backslashes));
                    parameters.push(value);
                    backslashes = 0;
                }
            }
        }
        parameters.extend(std::iter::repeat_n(u16::from(b'\\'), backslashes * 2));
        parameters.push(u16::from(b'"'));
    }
    if !parameters.is_empty() {
        parameters.push(0);
    }
    parameters
}

#[cfg(windows)]
pub fn launch_process_elevated(
    executable: &Path,
    arguments: &[OsString],
) -> Result<(), WindowsElevationError> {
    use std::mem::size_of;
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0};
    use windows_sys::Win32::System::Threading::{WaitForInputIdle, WaitForSingleObject};
    use windows_sys::Win32::UI::Shell::{ShellExecuteExW, SHELLEXECUTEINFOW};
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    if !executable.is_absolute() || !executable.is_file() {
        return Err(WindowsElevationError::InvalidExecutable);
    }
    let operation = "runas"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let directory = executable
        .parent()
        .ok_or(WindowsElevationError::InvalidExecutable)?
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let executable = executable
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let parameters = elevated_launch_parameters(arguments);
    let mut execute_info = SHELLEXECUTEINFOW {
        cbSize: size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: elevated_launch_mask(),
        hwnd: std::ptr::null_mut(),
        lpVerb: operation.as_ptr(),
        lpFile: executable.as_ptr(),
        lpParameters: parameters
            .first()
            .map_or(std::ptr::null(), |_| parameters.as_ptr()),
        lpDirectory: directory.as_ptr(),
        nShow: SW_SHOWNORMAL,
        ..Default::default()
    };
    if unsafe { ShellExecuteExW(&mut execute_info) } == 0 || execute_info.hProcess.is_null() {
        return Err(WindowsElevationError::LaunchFailed);
    }

    // The caller exits as soon as this function succeeds. Wait for the elevated GUI process to
    // create its message queue, then make sure it did not terminate during startup.
    unsafe {
        WaitForInputIdle(execute_info.hProcess, 10_000);
    }
    let child_exited = unsafe { WaitForSingleObject(execute_info.hProcess, 0) } == WAIT_OBJECT_0;
    unsafe {
        CloseHandle(execute_info.hProcess);
    }
    (!child_exited)
        .then_some(())
        .ok_or(WindowsElevationError::LaunchFailed)
}

#[cfg(not(windows))]
pub fn launch_process_elevated(
    _executable: &Path,
    _arguments: &[OsString],
) -> Result<(), WindowsElevationError> {
    Err(WindowsElevationError::UnsupportedOperatingSystem)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn running_executables_are_sorted_and_deduplicated_by_path() {
        let executables = normalize_running_executables(vec![
            WindowsExecutable {
                path: PathBuf::from(r"C:\Synthetic\zeta.exe"),
                name: "zeta.exe".into(),
                architecture: "x86_64".into(),
                running: true,
            },
            WindowsExecutable {
                path: PathBuf::from(r"C:\Synthetic\Alpha.exe"),
                name: "Alpha.exe".into(),
                architecture: "x86_64".into(),
                running: true,
            },
            WindowsExecutable {
                path: PathBuf::from(r"c:\synthetic\alpha.exe"),
                name: "Alpha.exe".into(),
                architecture: "x86_64".into(),
                running: true,
            },
        ]);

        assert_eq!(executables.len(), 2);
        assert_eq!(executables[0].name(), "Alpha.exe");
        assert_eq!(executables[1].name(), "zeta.exe");
    }
}

use glyphshift_controller_sdk::PluginError;
use std::path::Path;

pub(super) trait SuspendedProcess {
    fn process_id(&self) -> u32;
    fn resume(self: Box<Self>) -> Result<(), PluginError>;
}

pub(super) trait ProcessLauncher {
    fn launch_suspended(
        &mut self,
        executable: &Path,
    ) -> Result<Box<dyn SuspendedProcess>, PluginError>;
}

pub(super) struct SystemProcessLauncher;

#[cfg(windows)]
fn executable_entrypoint_rva(executable: &Path) -> Result<u32, PluginError> {
    use std::fs::File;
    use std::io::{Read, Seek, SeekFrom};

    let invalid = || PluginError::new("controlled_launch_entrypoint_unavailable");
    let mut file = File::open(executable).map_err(|_| invalid())?;
    let mut dos = [0_u8; 64];
    file.read_exact(&mut dos).map_err(|_| invalid())?;
    if &dos[..2] != b"MZ" {
        return Err(invalid());
    }
    let pe_offset = u32::from_le_bytes(dos[0x3c..0x40].try_into().map_err(|_| invalid())?);
    file.seek(SeekFrom::Start(pe_offset as u64))
        .map_err(|_| invalid())?;
    let mut headers = [0_u8; 44];
    file.read_exact(&mut headers).map_err(|_| invalid())?;
    if &headers[..4] != b"PE\0\0" {
        return Err(invalid());
    }
    let optional_size = u16::from_le_bytes([headers[20], headers[21]]);
    let optional_magic = u16::from_le_bytes([headers[24], headers[25]]);
    if optional_size < 20 || !matches!(optional_magic, 0x10b | 0x20b) {
        return Err(invalid());
    }
    let entrypoint = u32::from_le_bytes(headers[40..44].try_into().map_err(|_| invalid())?);
    (entrypoint != 0).then_some(entrypoint).ok_or_else(invalid)
}

#[cfg(windows)]
impl ProcessLauncher for SystemProcessLauncher {
    fn launch_suspended(
        &mut self,
        executable: &Path,
    ) -> Result<Box<dyn SuspendedProcess>, PluginError> {
        use std::mem::size_of;
        use std::os::windows::ffi::OsStrExt;
        use std::ptr::null;
        use windows_sys::Win32::Foundation::{
            CloseHandle, ERROR_ELEVATION_REQUIRED, GetLastError, HANDLE,
        };
        use windows_sys::Win32::System::Threading::{
            CREATE_SUSPENDED, CreateProcessW, PROCESS_INFORMATION, STARTUPINFOW,
        };

        if !executable.is_absolute() || !executable.is_file() {
            return Err(PluginError::new("controlled_launch_invalid_executable"));
        }

        let application = executable
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();
        let mut command_line = Vec::with_capacity(application.len() + 2);
        command_line.push(u16::from(b'\"'));
        command_line.extend(
            application
                .iter()
                .copied()
                .take(application.len().saturating_sub(1)),
        );
        command_line.push(u16::from(b'\"'));
        command_line.push(0);
        let directory = executable
            .parent()
            .ok_or_else(|| PluginError::new("controlled_launch_invalid_executable"))?
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();
        let mut startup = STARTUPINFOW {
            cb: size_of::<STARTUPINFOW>() as u32,
            ..Default::default()
        };
        let mut process = PROCESS_INFORMATION::default();
        let created = unsafe {
            CreateProcessW(
                application.as_ptr(),
                command_line.as_mut_ptr(),
                null(),
                null(),
                0,
                CREATE_SUSPENDED,
                null(),
                directory.as_ptr(),
                &mut startup,
                &mut process,
            )
        };
        if created == 0 {
            let error = unsafe { GetLastError() };
            return Err(PluginError::new(if error == ERROR_ELEVATION_REQUIRED {
                "controlled_launch_elevation_required"
            } else {
                "controlled_launch_failed"
            }));
        }
        if process.hProcess.is_null() || process.hThread.is_null() || process.dwProcessId == 0 {
            if !process.hThread.is_null() {
                unsafe { CloseHandle(process.hThread) };
            }
            if !process.hProcess.is_null() {
                unsafe { CloseHandle(process.hProcess) };
            }
            return Err(PluginError::new("controlled_launch_failed"));
        }

        let mut process = WindowsSuspendedProcess {
            process_id: process.dwProcessId,
            process: process.hProcess as HANDLE,
            thread: process.hThread as HANDLE,
            resumed: false,
        };
        process.wait_for_entrypoint_barrier(executable_entrypoint_rva(executable)?)?;
        Ok(Box::new(process))
    }
}

#[cfg(windows)]
struct WindowsSuspendedProcess {
    process_id: u32,
    process: windows_sys::Win32::Foundation::HANDLE,
    thread: windows_sys::Win32::Foundation::HANDLE,
    resumed: bool,
}

#[cfg(windows)]
impl WindowsSuspendedProcess {
    fn wait_for_entrypoint_barrier(&mut self, entrypoint_rva: u32) -> Result<(), PluginError> {
        use std::ffi::c_void;
        use std::mem::size_of;
        use std::ptr::null_mut;
        use std::thread;
        use std::time::{Duration, Instant};
        use windows_sys::Wdk::System::Threading::NtQueryInformationProcess;
        use windows_sys::Win32::System::Diagnostics::Debug::{
            CONTEXT, FlushInstructionCache, GetThreadContext, ReadProcessMemory, WriteProcessMemory,
        };
        use windows_sys::Win32::System::Threading::{
            PROCESS_BASIC_INFORMATION, ResumeThread, SuspendThread,
        };

        const INITIALIZATION_TIMEOUT: Duration = Duration::from_secs(15);
        const ENTRY_LOOP: [u8; 2] = [0xeb, 0xfe];

        // CREATE_SUSPENDED stops before user-mode loader initialization, where remote LoadLibraryW
        // calls can deadlock. Temporarily turn the executable entry point into a self-loop, let
        // Windows finish loader startup, then suspend there and restore the original bytes. This
        // keeps the process before its first application instruction while making the loader usable.
        let mut process_info = PROCESS_BASIC_INFORMATION::default();
        let status = unsafe {
            NtQueryInformationProcess(
                self.process,
                0,
                (&mut process_info as *mut PROCESS_BASIC_INFORMATION).cast(),
                size_of::<PROCESS_BASIC_INFORMATION>() as u32,
                null_mut(),
            )
        };
        if status < 0 || process_info.PebBaseAddress.is_null() {
            return Err(PluginError::new("controlled_launch_entrypoint_unavailable"));
        }

        let image_base_offset = if size_of::<usize>() == 8 {
            0x10usize
        } else {
            0x08usize
        };
        let image_base_address = (process_info.PebBaseAddress as usize)
            .checked_add(image_base_offset)
            .ok_or_else(|| PluginError::new("controlled_launch_entrypoint_unavailable"))?;
        let mut image_base = 0usize;
        let mut read = 0usize;
        if unsafe {
            ReadProcessMemory(
                self.process,
                image_base_address as *const c_void,
                (&mut image_base as *mut usize).cast(),
                size_of::<usize>(),
                &mut read,
            )
        } == 0
            || read != size_of::<usize>()
            || image_base == 0
        {
            return Err(PluginError::new("controlled_launch_entrypoint_unavailable"));
        }

        let entrypoint = image_base
            .checked_add(entrypoint_rva as usize)
            .ok_or_else(|| PluginError::new("controlled_launch_entrypoint_unavailable"))?;
        let mut original = [0u8; ENTRY_LOOP.len()];
        read = 0;
        if unsafe {
            ReadProcessMemory(
                self.process,
                entrypoint as *const c_void,
                original.as_mut_ptr().cast(),
                original.len(),
                &mut read,
            )
        } == 0
            || read != original.len()
        {
            return Err(PluginError::new("controlled_launch_entrypoint_unavailable"));
        }

        let mut written = 0usize;
        if unsafe {
            WriteProcessMemory(
                self.process,
                entrypoint as *const c_void,
                ENTRY_LOOP.as_ptr().cast(),
                ENTRY_LOOP.len(),
                &mut written,
            )
        } == 0
            || written != ENTRY_LOOP.len()
            || unsafe {
                FlushInstructionCache(self.process, entrypoint as *const c_void, ENTRY_LOOP.len())
            } == 0
        {
            return Err(PluginError::new(
                "controlled_launch_entrypoint_breakpoint_failed",
            ));
        }

        if unsafe { ResumeThread(self.thread) } == u32::MAX {
            return Err(PluginError::new("controlled_launch_initialization_failed"));
        }

        let deadline = Instant::now() + INITIALIZATION_TIMEOUT;
        loop {
            if Instant::now() >= deadline {
                return Err(PluginError::new("controlled_launch_initialization_timeout"));
            }
            thread::sleep(Duration::from_millis(1));
            if unsafe { SuspendThread(self.thread) } == u32::MAX {
                return Err(PluginError::new("controlled_launch_initialization_failed"));
            }

            let mut context = CONTEXT::default();
            #[cfg(target_arch = "x86_64")]
            {
                use windows_sys::Win32::System::Diagnostics::Debug::CONTEXT_CONTROL_AMD64;
                context.ContextFlags = CONTEXT_CONTROL_AMD64;
            }
            #[cfg(target_arch = "x86")]
            {
                use windows_sys::Win32::System::Diagnostics::Debug::CONTEXT_CONTROL_X86;
                context.ContextFlags = CONTEXT_CONTROL_X86;
            }
            if unsafe { GetThreadContext(self.thread, &mut context) } == 0 {
                return Err(PluginError::new(
                    "controlled_launch_entrypoint_context_failed",
                ));
            }

            #[cfg(target_arch = "x86_64")]
            let instruction_pointer = context.Rip as usize;
            #[cfg(target_arch = "x86")]
            let instruction_pointer = context.Eip as usize;

            if instruction_pointer == entrypoint {
                written = 0;
                if unsafe {
                    WriteProcessMemory(
                        self.process,
                        entrypoint as *const c_void,
                        original.as_ptr().cast(),
                        original.len(),
                        &mut written,
                    )
                } == 0
                    || written != original.len()
                    || unsafe {
                        FlushInstructionCache(
                            self.process,
                            entrypoint as *const c_void,
                            original.len(),
                        )
                    } == 0
                {
                    return Err(PluginError::new(
                        "controlled_launch_entrypoint_restore_failed",
                    ));
                }
                return Ok(());
            }

            if unsafe { ResumeThread(self.thread) } == u32::MAX {
                return Err(PluginError::new("controlled_launch_initialization_failed"));
            }
        }
    }
}
#[cfg(windows)]
impl SuspendedProcess for WindowsSuspendedProcess {
    fn process_id(&self) -> u32 {
        self.process_id
    }

    fn resume(mut self: Box<Self>) -> Result<(), PluginError> {
        use windows_sys::Win32::System::Threading::ResumeThread;

        if unsafe { ResumeThread(self.thread) } == u32::MAX {
            return Err(PluginError::new("controlled_launch_resume_failed"));
        }
        self.resumed = true;
        Ok(())
    }
}

#[cfg(windows)]
impl Drop for WindowsSuspendedProcess {
    fn drop(&mut self) {
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::Threading::TerminateProcess;

        if !self.resumed && !self.process.is_null() {
            unsafe {
                TerminateProcess(self.process, 1);
            }
        }
        unsafe {
            if !self.thread.is_null() {
                CloseHandle(self.thread);
            }
            if !self.process.is_null() {
                CloseHandle(self.process);
            }
        }
    }
}

#[cfg(not(windows))]
impl ProcessLauncher for SystemProcessLauncher {
    fn launch_suspended(
        &mut self,
        _executable: &Path,
    ) -> Result<Box<dyn SuspendedProcess>, PluginError> {
        Err(PluginError::new("unsupported_operating_system"))
    }
}

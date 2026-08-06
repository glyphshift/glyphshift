//! Generic Windows target discovery behind the isolated Controller protocol.

mod controller;
mod executable;
mod platform;
#[cfg(windows)]
mod remote;

pub use controller::WindowsController;
pub use executable::{
    current_process_is_elevated, foreground_windows_executable, inspect_windows_executable,
    launch_process_elevated, WindowsElevationError, WindowsExecutable,
};

#[cfg(test)]
#[path = "tests/mod.rs"]
mod tests;

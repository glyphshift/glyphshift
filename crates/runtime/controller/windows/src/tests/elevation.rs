use crate::current_process_is_elevated;
#[cfg(windows)]
use crate::executable::{elevated_launch_mask, elevated_launch_parameters};
#[cfg(windows)]
use std::ffi::OsString;

#[cfg(windows)]
#[test]
fn current_process_elevation_is_queryable_without_changing_process_state() {
    assert!(current_process_is_elevated().is_ok());
}

#[cfg(windows)]
#[test]
fn elevated_launch_waits_for_the_shell_handoff_and_keeps_the_child_handle() {
    use windows_sys::Win32::UI::Shell::{SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS};

    let mask = elevated_launch_mask();

    assert_ne!(mask & SEE_MASK_NOASYNC, 0);
    assert_ne!(mask & SEE_MASK_NOCLOSEPROCESS, 0);
}

#[cfg(windows)]
#[test]
fn elevated_launch_quotes_internal_arguments_as_one_windows_command_line() {
    let parameters = elevated_launch_parameters(&[
        OsString::from("--glyphshift-data-root"),
        OsString::from(r"<synthetic-root>\workspace"),
    ]);
    let parameters = String::from_utf16(&parameters[..parameters.len() - 1])
        .expect("synthetic parameters are valid UTF-16");

    assert_eq!(
        parameters,
        r#""--glyphshift-data-root" "<synthetic-root>\workspace""#
    );
}

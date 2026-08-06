#![cfg(windows)]

use glyphshift_adapter_native_abi::{NativeDecisionV1, NativeRuntimeHostV1, STATUS_OK};
use glyphshift_adapter_native_host::LoadedNativeAdapter;
use glyphshift_domain::Feature;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static OBSERVED: AtomicUsize = AtomicUsize::new(0);

#[link(name = "kernel32")]
extern "system" {
    fn WriteConsoleW(
        output: *mut core::ffi::c_void,
        buffer: *const core::ffi::c_void,
        length: u32,
        written: *mut u32,
        reserved: *const core::ffi::c_void,
    ) -> i32;
}

fn native_package() -> PathBuf {
    let executable = std::env::current_exe().expect("current test executable");
    executable
        .parent()
        .and_then(|deps| deps.parent())
        .expect("Cargo profile directory")
        .join("glyphshift_adapter_console_native.dll")
}

extern "C" fn decide(
    _context: *mut core::ffi::c_void,
    source: *const u16,
    source_len: u32,
    _text_out: *mut u16,
    _text_capacity: u32,
    _font_out: *mut u16,
    _font_capacity: u32,
) -> NativeDecisionV1 {
    let source = unsafe { std::slice::from_raw_parts(source, source_len as usize) };
    if String::from_utf16_lossy(source) == "Open" {
        OBSERVED.fetch_add(1, Ordering::AcqRel);
    }
    NativeDecisionV1 {
        status: STATUS_OK,
        generation: 1,
        decision_bits: 0,
        text_len: 0,
        font_len: 0,
    }
}

extern "C" fn source_characters(
    _context: *mut core::ffi::c_void,
    _output: *mut u16,
    _capacity: u32,
) -> u32 {
    0
}

#[test]
fn native_console_observes_write_console_and_deactivates_to_pass_through() {
    let package = unsafe {
        LoadedNativeAdapter::load(&native_package(), &glyphshift_adapter_console::descriptor())
            .expect("load verified Console observer")
    };
    let host = Box::leak(Box::new(NativeRuntimeHostV1 {
        struct_size: std::mem::size_of::<NativeRuntimeHostV1>() as u32,
        context: std::ptr::null_mut(),
        decide_utf16: decide,
        source_characters_utf16: source_characters,
    }));
    OBSERVED.store(0, Ordering::Release);

    package
        .activate(host, [], [Feature::TextObserve])
        .expect("activate Console package without observation");
    let text = "Open".encode_utf16().collect::<Vec<_>>();
    let mut written = 0_u32;
    unsafe {
        let _ = WriteConsoleW(
            (-1_isize) as *mut core::ffi::c_void,
            text.as_ptr().cast(),
            text.len() as u32,
            &mut written,
            std::ptr::null(),
        );
    }
    assert_eq!(OBSERVED.load(Ordering::Acquire), 0);

    package
        .activate(host, [Feature::TextObserve], [Feature::TextObserve])
        .expect("activate Console observer");

    unsafe {
        let _ = WriteConsoleW(
            (-1_isize) as *mut core::ffi::c_void,
            text.as_ptr().cast(),
            text.len() as u32,
            &mut written,
            std::ptr::null(),
        );
    }
    assert_eq!(OBSERVED.load(Ordering::Acquire), 1);

    let styled = "\u{1b}[0;90mOpen\u{1b}[m"
        .encode_utf16()
        .collect::<Vec<_>>();
    let control_only = "\u{1b}[13G\u{1b}[K".encode_utf16().collect::<Vec<_>>();
    unsafe {
        let _ = WriteConsoleW(
            (-1_isize) as *mut core::ffi::c_void,
            styled.as_ptr().cast(),
            styled.len() as u32,
            &mut written,
            std::ptr::null(),
        );
        let _ = WriteConsoleW(
            (-1_isize) as *mut core::ffi::c_void,
            control_only.as_ptr().cast(),
            control_only.len() as u32,
            &mut written,
            std::ptr::null(),
        );
    }
    assert_eq!(OBSERVED.load(Ordering::Acquire), 2);

    package.deactivate().expect("deactivate Console observer");
    unsafe {
        let _ = WriteConsoleW(
            (-1_isize) as *mut core::ffi::c_void,
            text.as_ptr().cast(),
            text.len() as u32,
            &mut written,
            std::ptr::null(),
        );
    }
    assert_eq!(OBSERVED.load(Ordering::Acquire), 2);
}

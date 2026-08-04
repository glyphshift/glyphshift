#![cfg(windows)]

use glyphshift_adapter_native_abi::{
    NativeDecisionV1, NativeRuntimeHostV1, STATUS_ACTIVATION_FAILED, STATUS_OK,
};
use glyphshift_adapter_native_host::{LoadedNativeAdapter, NativeHostError};
use glyphshift_domain::Feature;
use std::ffi::c_void;
use std::path::PathBuf;

fn native_package() -> PathBuf {
    let executable = std::env::current_exe().expect("current test executable");
    let profile = executable
        .parent()
        .and_then(|deps| deps.parent())
        .expect("Cargo profile directory");
    profile.join("glyphshift_adapter_gtk3_pango_native.dll")
}

extern "C" fn decide(
    _context: *mut c_void,
    _source: *const u16,
    _source_len: u32,
    _text_out: *mut u16,
    _text_capacity: u32,
    _font_out: *mut u16,
    _font_capacity: u32,
) -> NativeDecisionV1 {
    NativeDecisionV1 {
        status: STATUS_OK,
        generation: 1,
        decision_bits: 0,
        text_len: 0,
        font_len: 0,
    }
}

extern "C" fn source_characters(_context: *mut c_void, _output: *mut u16, _capacity: u32) -> u32 {
    0
}

fn host() -> &'static NativeRuntimeHostV1 {
    Box::leak(Box::new(NativeRuntimeHostV1 {
        struct_size: std::mem::size_of::<NativeRuntimeHostV1>() as u32,
        context: std::ptr::null_mut(),
        decide_utf16: decide,
        source_characters_utf16: source_characters,
    }))
}

#[test]
fn gtk3_pango_activation_rejects_a_process_without_required_exports() {
    let package = unsafe {
        LoadedNativeAdapter::load(
            &native_package(),
            &glyphshift_adapter_gtk3_pango::descriptor(),
        )
        .expect("load verified GTK 3 Pango package")
    };

    assert_eq!(
        package.activate(
            host(),
            [Feature::TextObserve, Feature::TextReplace],
            [Feature::TextObserve, Feature::TextReplace],
        ),
        Err(NativeHostError::PackageFailure(STATUS_ACTIVATION_FAILED))
    );
}

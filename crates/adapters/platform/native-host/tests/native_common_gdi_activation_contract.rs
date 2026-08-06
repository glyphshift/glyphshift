#![cfg(windows)]

use glyphshift_adapter_native_abi::{
    NativeDecisionV1, NativeRuntimeHostV1, DECISION_TEXT_REPLACE, STATUS_OK,
};
use glyphshift_adapter_native_host::LoadedNativeAdapter;
use glyphshift_domain::Feature;
use glyphshift_windows_host::{render_raw_draw_text, render_raw_text_out};
use std::path::PathBuf;

fn native_package(file_name: &str) -> PathBuf {
    let executable = std::env::current_exe().expect("current test executable");
    executable
        .parent()
        .and_then(|deps| deps.parent())
        .expect("Cargo profile directory")
        .join(file_name)
}

extern "C" fn decide(
    _context: *mut core::ffi::c_void,
    source: *const u16,
    source_len: u32,
    text_out: *mut u16,
    text_capacity: u32,
    _font_out: *mut u16,
    _font_capacity: u32,
) -> NativeDecisionV1 {
    let source = unsafe { std::slice::from_raw_parts(source, source_len as usize) };
    let source = String::from_utf16_lossy(source);
    let replacement = if source == "Open" {
        "Common API translated".encode_utf16().collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    if replacement.len() <= text_capacity as usize {
        unsafe {
            std::ptr::copy_nonoverlapping(replacement.as_ptr(), text_out, replacement.len());
        }
    }
    NativeDecisionV1 {
        status: STATUS_OK,
        generation: 1,
        decision_bits: (!replacement.is_empty()) as u32 * DECISION_TEXT_REPLACE,
        text_len: replacement.len() as u32,
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
fn native_common_gdi_001_text_out_and_draw_text_activate_as_distinct_seams() {
    let text_out_baseline = render_raw_text_out("Open").expect("baseline TextOutW render");
    let draw_text_baseline = render_raw_draw_text("Open").expect("baseline DrawTextW render");
    let host = Box::leak(Box::new(NativeRuntimeHostV1 {
        struct_size: std::mem::size_of::<NativeRuntimeHostV1>() as u32,
        context: std::ptr::null_mut(),
        decide_utf16: decide,
        source_characters_utf16: source_characters,
    }));

    let text_out = unsafe {
        LoadedNativeAdapter::load(
            &native_package("glyphshift_adapter_gdi_text_out_native.dll"),
            &glyphshift_adapter_gdi::text_out_descriptor(),
        )
        .expect("load verified TextOutW package")
    };
    text_out
        .activate(host, [Feature::TextReplace], [Feature::TextReplace])
        .expect("activate TextOutW package");
    assert_ne!(
        render_raw_text_out("Open")
            .expect("translated TextOutW render")
            .signature(),
        text_out_baseline.signature(),
    );
    text_out.deactivate().expect("deactivate TextOutW package");
    assert_eq!(
        render_raw_text_out("Open")
            .expect("restored TextOutW render")
            .signature(),
        text_out_baseline.signature(),
    );

    let draw_text = unsafe {
        LoadedNativeAdapter::load(
            &native_package("glyphshift_adapter_draw_text_native.dll"),
            &glyphshift_adapter_gdi::draw_text_descriptor(),
        )
        .expect("load verified DrawText package")
    };
    draw_text
        .activate(host, [Feature::TextReplace], [Feature::TextReplace])
        .expect("activate DrawText package");
    assert_ne!(
        render_raw_draw_text("Open")
            .expect("translated DrawTextW render")
            .signature(),
        draw_text_baseline.signature(),
    );
    draw_text.deactivate().expect("deactivate DrawText package");
    assert_eq!(
        render_raw_draw_text("Open")
            .expect("restored DrawTextW render")
            .signature(),
        draw_text_baseline.signature(),
    );

    std::mem::forget(text_out);
    std::mem::forget(draw_text);
}

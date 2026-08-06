#![cfg(windows)]

use glyphshift_adapter_native_abi::{
    NativeDecisionV1, NativeRuntimeHostV1, DECISION_FONT_SUBSTITUTE, DECISION_TEXT_REPLACE,
    STATUS_OK,
};
use glyphshift_adapter_native_host::LoadedNativeAdapter;
use glyphshift_domain::Feature;
use glyphshift_windows_host::render_raw_gdi_unicode;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU8, Ordering};

static MODE: AtomicU8 = AtomicU8::new(1);

fn native_package() -> PathBuf {
    let executable = std::env::current_exe().expect("current test executable");
    let profile = executable
        .parent()
        .and_then(|deps| deps.parent())
        .expect("Cargo profile directory");
    profile.join("glyphshift_adapter_gdi_native.dll")
}

extern "C" fn decide(
    _context: *mut core::ffi::c_void,
    source: *const u16,
    source_len: u32,
    text_out: *mut u16,
    text_capacity: u32,
    font_out: *mut u16,
    font_capacity: u32,
) -> NativeDecisionV1 {
    let source = unsafe { std::slice::from_raw_parts(source, source_len as usize) };
    let source = String::from_utf16_lossy(source);
    let replacement = if MODE.load(Ordering::Acquire) == 1 && source == "Open" {
        "File translated".encode_utf16().collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    if replacement.len() <= text_capacity as usize {
        unsafe {
            std::ptr::copy_nonoverlapping(replacement.as_ptr(), text_out, replacement.len());
        }
    }
    let font = if MODE.load(Ordering::Acquire) == 2 {
        "Consolas".encode_utf16().collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    if font.len() <= font_capacity as usize {
        unsafe {
            std::ptr::copy_nonoverlapping(font.as_ptr(), font_out, font.len());
        }
    }
    NativeDecisionV1 {
        status: STATUS_OK,
        generation: 1,
        decision_bits: if replacement.is_empty() {
            0
        } else {
            DECISION_TEXT_REPLACE
        } | if font.is_empty() {
            0
        } else {
            DECISION_FONT_SUBSTITUTE
        },
        text_len: replacement.len() as u32,
        font_len: font.len() as u32,
    }
}

extern "C" fn source_characters(
    _context: *mut core::ffi::c_void,
    output: *mut u16,
    capacity: u32,
) -> u32 {
    let units = "Open".encode_utf16().collect::<Vec<_>>();
    if units.len() <= capacity as usize {
        unsafe {
            std::ptr::copy_nonoverlapping(units.as_ptr(), output, units.len());
        }
    }
    units.len() as u32
}

#[test]
fn native_gdi_001_activates_text_and_font_independently_then_returns_to_pass_through() {
    let baseline = render_raw_gdi_unicode("Open").expect("baseline GDI render");
    let package = unsafe {
        LoadedNativeAdapter::load(&native_package(), &glyphshift_adapter_gdi::descriptor())
            .expect("load verified GDI package")
    };
    let host = Box::leak(Box::new(NativeRuntimeHostV1 {
        struct_size: std::mem::size_of::<NativeRuntimeHostV1>() as u32,
        context: std::ptr::null_mut(),
        decide_utf16: decide,
        source_characters_utf16: source_characters,
    }));

    assert_eq!(
        package
            .activate(host, [Feature::TextReplace], [Feature::TextReplace])
            .expect("activation must install the native detour"),
        [Feature::TextReplace]
    );
    let translated = render_raw_gdi_unicode("Open").expect("translated GDI render");
    assert_ne!(translated.signature(), baseline.signature());
    assert!(translated.ink_pixels() > baseline.ink_pixels());

    package.deactivate().expect("switch to pass-through");
    let restored = render_raw_gdi_unicode("Open").expect("pass-through GDI render");
    assert_eq!(restored.signature(), baseline.signature());

    MODE.store(2, Ordering::Release);
    assert_eq!(
        package
            .activate(host, [Feature::FontSubstitute], [Feature::FontSubstitute],)
            .expect("font-only activation must reuse the installed detour"),
        [Feature::FontSubstitute]
    );
    let substituted = render_raw_gdi_unicode("Open").expect("font-substituted GDI render");
    assert_ne!(substituted.signature(), baseline.signature());

    package
        .deactivate()
        .expect("return font path to pass-through");
    let final_pass = render_raw_gdi_unicode("Open").expect("final pass-through GDI render");
    assert_eq!(final_pass.signature(), baseline.signature());
}

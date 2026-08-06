#![cfg(windows)]

use glyphshift_adapter_native_abi::{
    NativeDecisionV1, NativeRuntimeHostV1, DECISION_FONT_SUBSTITUTE, DECISION_TEXT_REPLACE,
    STATUS_OK,
};
use glyphshift_adapter_native_host::LoadedNativeAdapter;
use glyphshift_domain::{Feature, FontDecision, Generation, RenderDecision, TextDecision};
use glyphshift_windows_host::render_gdiplus;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU8, Ordering};

static MODE: AtomicU8 = AtomicU8::new(1);

fn native_package() -> PathBuf {
    let executable = std::env::current_exe().expect("current test executable");
    let profile = executable
        .parent()
        .and_then(|deps| deps.parent())
        .expect("Cargo profile directory");
    profile.join("glyphshift_adapter_gdiplus_native.dll")
}

fn pass_decision() -> RenderDecision {
    RenderDecision {
        text: TextDecision::Keep,
        font: FontDecision::Keep,
        generation: Generation::new(1),
    }
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
    let family = if MODE.load(Ordering::Acquire) == 2 {
        "Consolas".encode_utf16().collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    if replacement.len() <= text_capacity as usize {
        unsafe {
            std::ptr::copy_nonoverlapping(replacement.as_ptr(), text_out, replacement.len());
        }
    }
    if family.len() <= font_capacity as usize {
        unsafe {
            std::ptr::copy_nonoverlapping(family.as_ptr(), font_out, family.len());
        }
    }
    NativeDecisionV1 {
        status: STATUS_OK,
        generation: 1,
        decision_bits: if replacement.is_empty() {
            0
        } else {
            DECISION_TEXT_REPLACE
        } | if family.is_empty() {
            0
        } else {
            DECISION_FONT_SUBSTITUTE
        },
        text_len: replacement.len() as u32,
        font_len: family.len() as u32,
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
fn native_gdiplus_001_activates_text_and_font_independently_then_passes_through() {
    let baseline = render_gdiplus(pass_decision()).expect("baseline GDI+ render");
    let package = unsafe {
        LoadedNativeAdapter::load(&native_package(), &glyphshift_adapter_gdiplus::descriptor())
            .expect("load verified GDI+ package")
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
            .expect("text activation must install the GDI+ detour"),
        [Feature::TextReplace]
    );
    let translated = render_gdiplus(pass_decision()).expect("translated GDI+ render");
    assert_ne!(translated.signature(), baseline.signature());
    assert!(translated.ink_pixels() > baseline.ink_pixels());

    package
        .deactivate()
        .expect("return text path to pass-through");
    assert_eq!(
        render_gdiplus(pass_decision())
            .expect("pass-through GDI+ render")
            .signature(),
        baseline.signature()
    );

    MODE.store(2, Ordering::Release);
    assert_eq!(
        package
            .activate(host, [Feature::FontSubstitute], [Feature::FontSubstitute],)
            .expect("font-only activation must reuse the installed GDI+ detour"),
        [Feature::FontSubstitute]
    );
    let substituted = render_gdiplus(pass_decision()).expect("font-substituted GDI+ render");
    assert_ne!(substituted.signature(), baseline.signature());

    package
        .deactivate()
        .expect("return font path to pass-through");
    assert_eq!(
        render_gdiplus(pass_decision())
            .expect("final pass-through GDI+ render")
            .signature(),
        baseline.signature()
    );
}

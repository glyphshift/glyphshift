#![cfg(windows)]
use glyphshift_adapter_native_abi::{
    font_scale_bits, NativeDecisionV1, NativeRuntimeHostV1, STATUS_OK,
};
use glyphshift_adapter_native_host::LoadedNativeAdapter;
use glyphshift_domain::Feature;
use glyphshift_windows_host::{render_raw_draw_text, render_raw_gdi_unicode, render_raw_text_out};
use std::sync::atomic::{AtomicU16, Ordering};
static PERCENT: AtomicU16 = AtomicU16::new(150);
extern "C" fn decide(
    _: *mut core::ffi::c_void,
    _: *const u16,
    _: u32,
    _: *mut u16,
    _: u32,
    _: *mut u16,
    _: u32,
) -> NativeDecisionV1 {
    NativeDecisionV1 {
        status: STATUS_OK,
        generation: 1,
        decision_bits: font_scale_bits(PERCENT.load(Ordering::Relaxed)),
        text_len: 0,
        font_len: 0,
    }
}
extern "C" fn characters(_: *mut core::ffi::c_void, _: *mut u16, _: u32) -> u32 {
    0
}
#[test]
fn native_gdi_scaling_changes_pixels_without_translation_and_restores_each_seam() {
    let host = Box::leak(Box::new(NativeRuntimeHostV1 {
        struct_size: std::mem::size_of::<NativeRuntimeHostV1>() as u32,
        context: std::ptr::null_mut(),
        decide_utf16: decide,
        source_characters_utf16: characters,
    }));
    let profile = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_owned();
    for (file, descriptor, render) in [
        (
            "glyphshift_adapter_gdi_native.dll",
            glyphshift_adapter_gdi::descriptor(),
            render_raw_gdi_unicode
                as fn(&str) -> Result<glyphshift_windows_host::PixelEvidence, String>,
        ),
        (
            "glyphshift_adapter_gdi_text_out_native.dll",
            glyphshift_adapter_gdi::text_out_descriptor(),
            render_raw_text_out
                as fn(&str) -> Result<glyphshift_windows_host::PixelEvidence, String>,
        ),
        (
            "glyphshift_adapter_draw_text_native.dll",
            glyphshift_adapter_gdi::draw_text_descriptor(),
            render_raw_draw_text
                as fn(&str) -> Result<glyphshift_windows_host::PixelEvidence, String>,
        ),
    ] {
        let baseline = render("Scale").unwrap().signature();
        let package =
            unsafe { LoadedNativeAdapter::load(&profile.join(file), &descriptor).unwrap() };
        package
            .activate(host, [Feature::FontScale], [Feature::FontScale])
            .unwrap();
        PERCENT.store(150, Ordering::Relaxed);
        let larger = render("Scale").unwrap().signature();
        assert_ne!(larger, baseline, "{file} must change rendered pixels");
        PERCENT.store(75, Ordering::Relaxed);
        let smaller = render("Scale").unwrap().signature();
        assert_ne!(smaller, larger);
        assert_ne!(smaller, baseline);
        PERCENT.store(100, Ordering::Relaxed);
        assert_eq!(render("Scale").unwrap().signature(), baseline);
        package.deactivate().unwrap();
        PERCENT.store(150, Ordering::Relaxed);
        assert_eq!(render("Scale").unwrap().signature(), baseline);
        std::mem::forget(package);
    }
}

#![cfg(windows)]
use glyphshift_adapter_native_abi::{NativeDecisionV1, NativeRuntimeHostV1, STATUS_OK};
use glyphshift_adapter_native_host::LoadedNativeAdapter;
use glyphshift_domain::Feature;
use glyphshift_windows_host::render_raw_gdi_unicode;
use std::sync::atomic::{AtomicUsize, Ordering};
static CALLS: AtomicUsize = AtomicUsize::new(0);
extern "C" fn decide(
    _: *mut core::ffi::c_void,
    _: *const u16,
    _: u32,
    _: *mut u16,
    _: u32,
    _: *mut u16,
    _: u32,
) -> NativeDecisionV1 {
    if CALLS.fetch_add(1, Ordering::SeqCst) == 0 {
        let _ = render_raw_gdi_unicode("Nested");
        let _ = render_raw_gdi_unicode("Nested again");
    }
    NativeDecisionV1 {
        status: STATUS_OK,
        generation: 1,
        decision_bits: 0,
        text_len: 0,
        font_len: 0,
    }
}
extern "C" fn characters(_: *mut core::ffi::c_void, _: *mut u16, _: u32) -> u32 {
    0
}
#[test]
fn repeated_nested_gdi_draws_do_not_reenter_translation() {
    let path = std::env::var_os("GLYPHSHIFT_REENTRY_ADAPTER")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::env::current_exe()
                .unwrap()
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .join("glyphshift_adapter_gdi_native.dll")
        });
    let adapter =
        unsafe { LoadedNativeAdapter::load(&path, &glyphshift_adapter_gdi::descriptor()).unwrap() };
    let host = Box::leak(Box::new(NativeRuntimeHostV1 {
        struct_size: size_of::<NativeRuntimeHostV1>() as u32,
        context: std::ptr::null_mut(),
        decide_utf16: decide,
        source_characters_utf16: characters,
    }));
    adapter
        .activate(host, [Feature::TextReplace], [Feature::TextReplace])
        .unwrap();
    let result = render_raw_gdi_unicode("Outer");
    adapter.deactivate().unwrap();
    assert!(result.is_ok());
    assert_eq!(
        CALLS.load(Ordering::SeqCst),
        1,
        "nested original draws must never reenter translation"
    );
}

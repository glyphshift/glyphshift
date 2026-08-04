#![cfg(windows)]

use glyphshift_adapter_native_abi::{
    NativeDecisionV1, NativeRuntimeHostV1, DECISION_TEXT_REPLACE, STATUS_INVALID_HOST, STATUS_OK,
};
use glyphshift_adapter_native_host::LoadedNativeAdapter;
use glyphshift_domain::Feature;
use glyphshift_windows_host::{render_raw_direct2d_text, render_raw_direct2d_wic_text};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};

static MODE: AtomicU8 = AtomicU8::new(0);
static OBSERVED: AtomicUsize = AtomicUsize::new(0);
const REPLACEMENT: &str = "WWWWWWWWWWWW";

fn native_package() -> PathBuf {
    let executable = std::env::current_exe().expect("current test executable");
    let profile = executable
        .parent()
        .and_then(|deps| deps.parent())
        .expect("Cargo profile directory");
    profile.join("glyphshift_adapter_direct2d_native.dll")
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
    if source == "Open" {
        OBSERVED.fetch_add(1, Ordering::AcqRel);
    }
    let mode = MODE.load(Ordering::Acquire);
    if mode == 2 {
        return NativeDecisionV1 {
            status: STATUS_INVALID_HOST,
            generation: 1,
            decision_bits: 0,
            text_len: 0,
            font_len: 0,
        };
    }
    let replacement = if mode == 1 && source == "Open" {
        REPLACEMENT.encode_utf16().collect::<Vec<_>>()
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
        decision_bits: if replacement.is_empty() {
            0
        } else {
            DECISION_TEXT_REPLACE
        },
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
fn native_direct2d_observes_replaces_fails_open_and_deactivates() {
    let baseline = render_raw_direct2d_text("Open").expect("baseline Direct2D draw");
    let expected = render_raw_direct2d_text(REPLACEMENT).expect("replacement Direct2D draw");
    assert_ne!(baseline.signature(), expected.signature());
    let wic_baseline = render_raw_direct2d_wic_text("Open").expect("baseline WIC Direct2D draw");
    let wic_expected =
        render_raw_direct2d_wic_text(REPLACEMENT).expect("replacement WIC Direct2D draw");
    assert_ne!(wic_baseline.signature(), wic_expected.signature());

    let package = unsafe {
        LoadedNativeAdapter::load(
            &native_package(),
            &glyphshift_adapter_direct2d::descriptor(),
        )
        .expect("load verified Direct2D package")
    };
    let host = Box::leak(Box::new(NativeRuntimeHostV1 {
        struct_size: std::mem::size_of::<NativeRuntimeHostV1>() as u32,
        context: std::ptr::null_mut(),
        decide_utf16: decide,
        source_characters_utf16: source_characters,
    }));

    MODE.store(0, Ordering::Release);
    OBSERVED.store(0, Ordering::Release);
    assert_eq!(
        package
            .activate(host, [Feature::TextObserve], [Feature::TextObserve])
            .expect("observe activation installs the Direct2D detour"),
        [Feature::TextObserve]
    );
    assert_eq!(
        render_raw_direct2d_text("Open").expect("observed Direct2D draw"),
        baseline
    );
    assert_eq!(
        render_raw_direct2d_wic_text("Open").expect("observed WIC Direct2D draw"),
        wic_baseline
    );
    assert_eq!(OBSERVED.load(Ordering::Acquire), 2);
    package.deactivate().expect("deactivate observation");

    MODE.store(1, Ordering::Release);
    assert_eq!(
        package
            .activate(host, [Feature::TextReplace], [Feature::TextReplace])
            .expect("replace activation reuses the Direct2D detour"),
        [Feature::TextReplace]
    );
    assert_eq!(
        render_raw_direct2d_text("Open").expect("translated Direct2D draw"),
        expected
    );
    assert_eq!(
        render_raw_direct2d_wic_text("Open").expect("translated WIC Direct2D draw"),
        wic_expected
    );
    package.deactivate().expect("deactivate replacement");
    assert_eq!(
        render_raw_direct2d_text("Open").expect("pass-through after deactivation"),
        baseline
    );
    assert_eq!(
        render_raw_direct2d_wic_text("Open").expect("WIC pass-through after deactivation"),
        wic_baseline
    );

    MODE.store(2, Ordering::Release);
    package
        .activate(host, [Feature::TextReplace], [Feature::TextReplace])
        .expect("reactivate for fail-open decision");
    assert_eq!(
        render_raw_direct2d_text("Open").expect("failed decision remains pass-through"),
        baseline
    );
    assert_eq!(
        render_raw_direct2d_wic_text("Open").expect("failed WIC decision remains pass-through"),
        wic_baseline
    );
    package.deactivate().expect("final deactivation");
}

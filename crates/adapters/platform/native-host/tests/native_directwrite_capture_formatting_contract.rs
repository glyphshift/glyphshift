#![cfg(windows)]

use glyphshift_adapter_native_abi::{
    NativeDecisionV1, NativeRuntimeHostV1, NativeTextEventV1, NativeTextHostV1,
    DECISION_TEXT_REPLACE, STATUS_INVALID_HOST, STATUS_OK, TEXT_EVENT_DRAW, TEXT_HOST_VERSION_V1,
};
use glyphshift_adapter_native_host::LoadedNativeAdapter;
use glyphshift_domain::Feature;
use glyphshift_windows_host::{
    render_raw_directwrite_formatted_layout, render_raw_directwrite_layout_sequence,
    DirectWriteLayoutStyle,
};
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering};

static MODE: AtomicU8 = AtomicU8::new(0);
static OBSERVED: AtomicUsize = AtomicUsize::new(0);
static SCOPES: AtomicUsize = AtomicUsize::new(0);
static DEPTH: AtomicUsize = AtomicUsize::new(0);
static COMPLETE_DRAW: AtomicBool = AtomicBool::new(false);
const SOURCE: &str = "office";
const TRANSLATION: &str = "打开合成";
const UPDATED_TRANSLATION: &str = "合成设置已更新";

extern "C" fn decide(
    _context: *mut core::ffi::c_void,
    source: *const u16,
    length: u32,
    output: *mut u16,
    capacity: u32,
    _font: *mut u16,
    _font_capacity: u32,
) -> NativeDecisionV1 {
    let source = unsafe { std::slice::from_raw_parts(source, length as usize) };
    if String::from_utf16_lossy(source) == SOURCE {
        OBSERVED.fetch_add(1, Ordering::AcqRel);
    }
    let mode = MODE.load(Ordering::Acquire);
    let mut result = NativeDecisionV1 {
        status: if mode == 2 {
            STATUS_INVALID_HOST
        } else {
            STATUS_OK
        },
        generation: if mode == 3 { 2 } else { 1 },
        decision_bits: 0,
        text_len: 0,
        font_len: 0,
    };
    if mode == 1 || mode == 3 {
        let translation = if mode == 3 {
            UPDATED_TRANSLATION
        } else {
            TRANSLATION
        }
        .encode_utf16()
        .collect::<Vec<_>>();
        if translation.len() <= capacity as usize {
            unsafe {
                std::ptr::copy_nonoverlapping(translation.as_ptr(), output, translation.len())
            };
            result.decision_bits = DECISION_TEXT_REPLACE;
            result.text_len = translation.len() as u32;
        }
    }
    result
}

extern "C" fn structured_decide(
    context: *mut core::ffi::c_void,
    event: *const NativeTextEventV1,
    output: *mut u16,
    capacity: u32,
    font: *mut u16,
    font_capacity: u32,
) -> NativeDecisionV1 {
    let event = unsafe { &*event };
    COMPLETE_DRAW.store(event.kind == TEXT_EVENT_DRAW, Ordering::Release);
    decide(
        context,
        event.source,
        event.source_len,
        output,
        capacity,
        font,
        font_capacity,
    )
}

extern "C" fn enter_scope(_context: *mut core::ffi::c_void) -> u64 {
    DEPTH.fetch_add(1, Ordering::AcqRel);
    SCOPES.fetch_add(1, Ordering::AcqRel) as u64 + 1
}

extern "C" fn leave_scope(_context: *mut core::ffi::c_void, _token: u64) {
    DEPTH.fetch_sub(1, Ordering::AcqRel);
}

extern "C" fn characters(
    _context: *mut core::ffi::c_void,
    _output: *mut u16,
    _capacity: u32,
) -> u32 {
    0
}

#[test]
fn formatted_layouts_observe_replace_uniform_typography_and_preserve_local_styles() {
    let cases = [
        (DirectWriteLayoutStyle::Typography, false),
        (DirectWriteLayoutStyle::Typography, true),
        (DirectWriteLayoutStyle::UpdatedUniformTypography, false),
        (DirectWriteLayoutStyle::UpdatedUniformTypography, true),
        (DirectWriteLayoutStyle::LocalFontSize, false),
        (DirectWriteLayoutStyle::LocalFontSize, true),
        (DirectWriteLayoutStyle::LocalTypography, false),
        (DirectWriteLayoutStyle::LocalTypography, true),
        (DirectWriteLayoutStyle::LocalCharacterSpacing, false),
        (DirectWriteLayoutStyle::LocalCharacterSpacing, true),
    ];
    let baselines = cases.map(|(style, compatible)| {
        let baseline = render_raw_directwrite_formatted_layout(SOURCE, compatible, style).unwrap();
        let translated =
            render_raw_directwrite_formatted_layout(TRANSLATION, compatible, style).unwrap();
        assert_ne!(baseline.signature(), translated.signature());
        baseline
    });
    let translated = cases.map(|(style, compatible)| {
        render_raw_directwrite_formatted_layout(TRANSLATION, compatible, style).unwrap()
    });
    let updated = cases.map(|(style, compatible)| {
        render_raw_directwrite_formatted_layout(UPDATED_TRANSLATION, compatible, style).unwrap()
    });
    let empty_baseline =
        render_raw_directwrite_formatted_layout(SOURCE, false, DirectWriteLayoutStyle::ReusedEmpty)
            .unwrap();
    let executable = std::env::current_exe().unwrap();
    let package_path = executable
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("glyphshift_adapter_directwrite_native.dll");
    let package = unsafe {
        LoadedNativeAdapter::load(&package_path, &glyphshift_adapter_directwrite::descriptor())
    }
    .expect("load DirectWrite adapter");
    let host = Box::leak(Box::new(NativeRuntimeHostV1 {
        struct_size: size_of::<NativeRuntimeHostV1>() as u32,
        context: std::ptr::null_mut(),
        decide_utf16: decide,
        source_characters_utf16: characters,
    }));
    let extended = Box::leak(Box::new(NativeTextHostV1 {
        struct_size: size_of::<NativeTextHostV1>() as u32,
        version: TEXT_HOST_VERSION_V1,
        context: host.context,
        decide_text: structured_decide,
        enter_scope: Some(enter_scope),
        leave_scope: Some(leave_scope),
    }));
    for structured in [false, true] {
        package
            .activate_with_text_host(
                host,
                structured.then_some(&*extended),
                [Feature::TextObserve, Feature::TextReplace],
                [Feature::TextObserve, Feature::TextReplace],
            )
            .expect("activate simultaneous capture and replacement");
        MODE.store(1, Ordering::Release);
        OBSERVED.store(0, Ordering::Release);
        SCOPES.store(0, Ordering::Release);
        let empty = render_raw_directwrite_formatted_layout(
            SOURCE,
            false,
            DirectWriteLayoutStyle::ReusedEmpty,
        )
        .unwrap();
        assert_eq!(
            empty, empty_baseline,
            "reused empty layouts must not paint a previous source's translation"
        );
        assert_eq!(
            OBSERVED.load(Ordering::Acquire),
            0,
            "empty layouts have no source to observe"
        );
        assert_eq!(
            SCOPES.load(Ordering::Acquire),
            0,
            "empty layouts have no replacement scope"
        );
        for (index, (style, compatible)) in cases.iter().copied().enumerate() {
            for mode in [0, 1, 2] {
                MODE.store(mode, Ordering::Release);
                OBSERVED.store(0, Ordering::Release);
                SCOPES.store(0, Ordering::Release);
                COMPLETE_DRAW.store(false, Ordering::Release);
                let pixels =
                    render_raw_directwrite_formatted_layout(SOURCE, compatible, style).unwrap();
                let supported = matches!(
                    style,
                    DirectWriteLayoutStyle::Typography
                        | DirectWriteLayoutStyle::UpdatedUniformTypography
                );
                let expected = if supported && mode == 1 {
                    &translated[index]
                } else {
                    &baselines[index]
                };
                assert_eq!(&pixels, expected, "translated uniform typography must draw with the source layout's formatting: {style:?}, compatible={compatible}, mode={mode}");
                assert_eq!(OBSERVED.load(Ordering::Acquire), 1, "draw must still publish its source: {style:?}, compatible={compatible}, mode={mode}");
                assert_eq!(DEPTH.load(Ordering::Acquire), 0, "source scope must close");
                assert_eq!(
                    SCOPES.load(Ordering::Acquire),
                    usize::from(supported && structured),
                    "only supported formatting may claim subordinate rendering"
                );
                assert_eq!(COMPLETE_DRAW.load(Ordering::Acquire), structured);
            }
        }
        for (index, (style, compatible)) in cases.iter().copied().enumerate().take(4) {
            let frames =
                render_raw_directwrite_layout_sequence(SOURCE, compatible, style, 3, |frame| {
                    MODE.store([1, 3, 0][frame], Ordering::Release);
                })
                .unwrap();
            assert_eq!(frames, [translated[index], updated[index], baselines[index]], "the same target layout must support publication updates and removing a translation");
        }
        let restored = render_raw_directwrite_layout_sequence(
            SOURCE,
            false,
            DirectWriteLayoutStyle::Typography,
            2,
            |frame| {
                MODE.store(1, Ordering::Release);
                if frame == 1 {
                    package.deactivate().unwrap();
                }
            },
        )
        .unwrap();
        assert_eq!(
            restored,
            [translated[0], baselines[0]],
            "deactivation must restore the target's unmodified layout"
        );
        OBSERVED.store(0, Ordering::Release);
        let pixels = render_raw_directwrite_formatted_layout(
            SOURCE,
            false,
            DirectWriteLayoutStyle::Typography,
        )
        .unwrap();
        assert_eq!(pixels, baselines[0]);
        assert_eq!(
            OBSERVED.load(Ordering::Acquire),
            0,
            "deactivation stops capture"
        );
    }
}

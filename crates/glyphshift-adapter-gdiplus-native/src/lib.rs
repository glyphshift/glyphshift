//! Native `GdipDrawString` package for the first-party GDI+ Adapter.

use glyphshift_adapter_gdiplus::ADAPTER_ID;
use glyphshift_adapter_native_abi::{
    DecideUtf16V1, NativeAdapterApiV1, NativeAdapterDescriptorV1, NativeDecisionV1,
    NativeNegotiationV1, NativeRuntimeHostV1, ARCH_X86, ARCH_X86_64, DECISION_FONT_SUBSTITUTE,
    DECISION_TEXT_REPLACE, FEATURE_FONT_SUBSTITUTE, FEATURE_TEXT_OBSERVE, FEATURE_TEXT_REPLACE,
    PLATFORM_WINDOWS, STATUS_ACTIVATION_FAILED, STATUS_INVALID_HOST, STATUS_OK,
    STATUS_UNAUTHORIZED_FEATURE, STATUS_UNSUPPORTED_FEATURE,
};
use retour::GenericDetour;
use std::cell::Cell;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{OnceLock, RwLock};
use windows::core::{s, w};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};

const SUPPORTED_FEATURES: u64 =
    FEATURE_TEXT_OBSERVE | FEATURE_TEXT_REPLACE | FEATURE_FONT_SUBSTITUTE;
const MAX_TEXT_UNITS: usize = 16 * 1024;
const MAX_FONT_UNITS: usize = 63;

type FnGdipDrawString = unsafe extern "system" fn(
    *mut core::ffi::c_void,
    *const u16,
    i32,
    *mut core::ffi::c_void,
    *const core::ffi::c_void,
    *mut core::ffi::c_void,
    *mut core::ffi::c_void,
) -> i32;
type FnGdipGetFontSize = unsafe extern "system" fn(*mut core::ffi::c_void, *mut f32) -> i32;
type FnGdipGetFontStyle = unsafe extern "system" fn(*mut core::ffi::c_void, *mut i32) -> i32;
type FnGdipGetFontUnit = unsafe extern "system" fn(*mut core::ffi::c_void, *mut i32) -> i32;
type FnGdipCreateFontFamilyFromName = unsafe extern "system" fn(
    *const u16,
    *mut core::ffi::c_void,
    *mut *mut core::ffi::c_void,
) -> i32;
type FnGdipDeleteFontFamily = unsafe extern "system" fn(*mut core::ffi::c_void) -> i32;
type FnGdipCreateFont = unsafe extern "system" fn(
    *mut core::ffi::c_void,
    f32,
    i32,
    i32,
    *mut *mut core::ffi::c_void,
) -> i32;
type FnGdipDeleteFont = unsafe extern "system" fn(*mut core::ffi::c_void) -> i32;
type RawProc = unsafe extern "system" fn() -> isize;

#[derive(Clone, Copy)]
struct HostBridge {
    context: usize,
    decide_utf16: DecideUtf16V1,
}

#[derive(Clone, Copy)]
struct GdiPlusFunctions {
    get_font_size: FnGdipGetFontSize,
    get_font_style: FnGdipGetFontStyle,
    get_font_unit: FnGdipGetFontUnit,
    create_family: FnGdipCreateFontFamilyFromName,
    delete_family: FnGdipDeleteFontFamily,
    create_font: FnGdipCreateFont,
    delete_font: FnGdipDeleteFont,
}

struct DecisionBuffers {
    decision: NativeDecisionV1,
    text: Vec<u16>,
    font: Vec<u16>,
}

static ACTIVE_FEATURES: AtomicU64 = AtomicU64::new(0);
static HOST: OnceLock<RwLock<HostBridge>> = OnceLock::new();
static HOOK: OnceLock<GenericDetour<FnGdipDrawString>> = OnceLock::new();
static FUNCTIONS: OnceLock<GdiPlusFunctions> = OnceLock::new();

thread_local! {
    static IN_CALLBACK: Cell<bool> = const { Cell::new(false) };
}

struct CallbackGuard;

impl CallbackGuard {
    fn enter() -> Option<Self> {
        IN_CALLBACK.with(|active| (!active.replace(true)).then_some(Self))
    }
}

impl Drop for CallbackGuard {
    fn drop(&mut self) {
        IN_CALLBACK.with(|active| active.set(false));
    }
}

extern "C" fn negotiate_features(requested: u64, granted: u64) -> NativeNegotiationV1 {
    if requested & !SUPPORTED_FEATURES != 0 {
        return negotiation_error(STATUS_UNSUPPORTED_FEATURE);
    }
    if requested & !granted != 0 {
        return negotiation_error(STATUS_UNAUTHORIZED_FEATURE);
    }
    NativeNegotiationV1 {
        status: STATUS_OK,
        active_feature_bits: requested,
    }
}

fn negotiation_error(status: i32) -> NativeNegotiationV1 {
    NativeNegotiationV1 {
        status,
        active_feature_bits: 0,
    }
}

extern "C" fn activate(
    host: *const NativeRuntimeHostV1,
    requested: u64,
    granted: u64,
) -> NativeNegotiationV1 {
    if host.is_null()
        || unsafe { (*host).struct_size } != std::mem::size_of::<NativeRuntimeHostV1>() as u32
    {
        return negotiation_error(STATUS_INVALID_HOST);
    }
    let negotiated = negotiate_features(requested, granted);
    if negotiated.status != STATUS_OK {
        return negotiated;
    }
    let host = unsafe { *host };
    let bridge = HostBridge {
        context: host.context as usize,
        decide_utf16: host.decide_utf16,
    };
    let host_ready = if let Some(current) = HOST.get() {
        current.write().map(|mut current| *current = bridge).is_ok()
    } else {
        HOST.set(RwLock::new(bridge)).is_ok()
    };
    if !host_ready {
        return negotiation_error(STATUS_ACTIVATION_FAILED);
    }
    if HOOK.get().is_none() && unsafe { install_hook() }.is_err() {
        return negotiation_error(STATUS_ACTIVATION_FAILED);
    }
    ACTIVE_FEATURES.store(negotiated.active_feature_bits, Ordering::Release);
    negotiated
}

extern "C" fn deactivate() -> i32 {
    ACTIVE_FEATURES.store(0, Ordering::Release);
    STATUS_OK
}

fn decide(source: &str) -> Option<DecisionBuffers> {
    let host = *HOST.get()?.read().ok()?;
    let source = source.encode_utf16().collect::<Vec<_>>();
    let mut text = vec![0_u16; MAX_TEXT_UNITS];
    let mut font = vec![0_u16; MAX_FONT_UNITS];
    let decision = (host.decide_utf16)(
        host.context as *mut core::ffi::c_void,
        source.as_ptr(),
        source.len() as u32,
        text.as_mut_ptr(),
        text.len() as u32,
        font.as_mut_ptr(),
        font.len() as u32,
    );
    if decision.status != STATUS_OK
        || decision.text_len as usize > text.len()
        || decision.font_len as usize > font.len()
    {
        return None;
    }
    text.truncate(decision.text_len as usize);
    font.truncate(decision.font_len as usize);
    Some(DecisionBuffers {
        decision,
        text,
        font,
    })
}

unsafe fn read_text(text: *const u16, length: i32) -> Option<String> {
    if text.is_null() {
        return None;
    }
    let length = if length < 0 {
        let mut length = 0_usize;
        while length < MAX_TEXT_UNITS && *text.add(length) != 0 {
            length += 1;
        }
        (length < MAX_TEXT_UNITS).then_some(length)?
    } else {
        usize::try_from(length).ok()?.min(MAX_TEXT_UNITS)
    };
    (length > 0).then(|| String::from_utf16_lossy(std::slice::from_raw_parts(text, length)))
}

fn normalize_layout_padding(source: &str) -> &str {
    let trimmed = source.trim();
    let Some(without_terminal_dot) = trimmed.strip_suffix('.') else {
        return trimmed;
    };
    let semantic_label = without_terminal_dot.trim_end();
    if semantic_label.len() < without_terminal_dot.len() {
        semantic_label
    } else {
        trimmed
    }
}

unsafe fn replacement_font(
    original: *mut core::ffi::c_void,
    family_units: &[u16],
) -> Option<(*mut core::ffi::c_void, *mut core::ffi::c_void)> {
    let functions = FUNCTIONS.get()?;
    let mut size = 0_f32;
    if (functions.get_font_size)(original, &mut size) != 0 || size <= 0.0 {
        return None;
    }
    let mut style = 0_i32;
    let mut unit = 2_i32;
    if (functions.get_font_style)(original, &mut style) != 0
        || (functions.get_font_unit)(original, &mut unit) != 0
    {
        return None;
    }
    let family_name = family_units
        .iter()
        .copied()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let mut family = std::ptr::null_mut();
    if (functions.create_family)(family_name.as_ptr(), std::ptr::null_mut(), &mut family) != 0
        || family.is_null()
    {
        return None;
    }
    let mut font = std::ptr::null_mut();
    if (functions.create_font)(family, size, style, unit, &mut font) != 0 || font.is_null() {
        (functions.delete_family)(family);
        return None;
    }
    Some((font, family))
}

unsafe fn delete_replacement_font(font: *mut core::ffi::c_void, family: *mut core::ffi::c_void) {
    if let Some(functions) = FUNCTIONS.get() {
        (functions.delete_font)(font);
        (functions.delete_family)(family);
    }
}

unsafe extern "system" fn gdip_draw_string_detour(
    graphics: *mut core::ffi::c_void,
    text: *const u16,
    length: i32,
    font: *mut core::ffi::c_void,
    layout: *const core::ffi::c_void,
    format: *mut core::ffi::c_void,
    brush: *mut core::ffi::c_void,
) -> i32 {
    let Some(original) = HOOK.get() else {
        return 1;
    };
    if ACTIVE_FEATURES.load(Ordering::Acquire) == 0 {
        return original.call(graphics, text, length, font, layout, format, brush);
    }
    let Some(_guard) = CallbackGuard::enter() else {
        return original.call(graphics, text, length, font, layout, format, brush);
    };
    let Some(source) = read_text(text, length) else {
        return original.call(graphics, text, length, font, layout, format, brush);
    };
    let decision = std::panic::catch_unwind(|| decide(normalize_layout_padding(&source)))
        .ok()
        .flatten();
    let Some(decision) = decision else {
        return original.call(graphics, text, length, font, layout, format, brush);
    };
    let active = ACTIVE_FEATURES.load(Ordering::Acquire);
    let replace_text = active & FEATURE_TEXT_REPLACE != 0
        && decision.decision.decision_bits & DECISION_TEXT_REPLACE != 0;
    let replace_font = active & FEATURE_FONT_SUBSTITUTE != 0
        && decision.decision.decision_bits & DECISION_FONT_SUBSTITUTE != 0;
    let (draw_text, draw_length) = if replace_text {
        (decision.text.as_ptr(), decision.text.len() as i32)
    } else {
        (text, length)
    };
    let created_font = replace_font
        .then(|| replacement_font(font, &decision.font))
        .flatten();
    let draw_font = created_font.map_or(font, |(font, _)| font);
    let status = original.call(
        graphics,
        draw_text,
        draw_length,
        draw_font,
        layout,
        format,
        brush,
    );
    if let Some((font, family)) = created_font {
        delete_replacement_font(font, family);
    }
    status
}

unsafe fn install_hook() -> Result<(), ()> {
    let module = LoadLibraryW(w!("gdiplus.dll")).map_err(|_| ())?;
    let draw = GetProcAddress(module, s!("GdipDrawString")).ok_or(())?;
    let target: FnGdipDrawString = std::mem::transmute(draw);
    FUNCTIONS
        .set(GdiPlusFunctions {
            get_font_size: std::mem::transmute::<RawProc, FnGdipGetFontSize>(
                GetProcAddress(module, s!("GdipGetFontSize")).ok_or(())?,
            ),
            get_font_style: std::mem::transmute::<RawProc, FnGdipGetFontStyle>(
                GetProcAddress(module, s!("GdipGetFontStyle")).ok_or(())?,
            ),
            get_font_unit: std::mem::transmute::<RawProc, FnGdipGetFontUnit>(
                GetProcAddress(module, s!("GdipGetFontUnit")).ok_or(())?,
            ),
            create_family: std::mem::transmute::<RawProc, FnGdipCreateFontFamilyFromName>(
                GetProcAddress(module, s!("GdipCreateFontFamilyFromName")).ok_or(())?,
            ),
            delete_family: std::mem::transmute::<RawProc, FnGdipDeleteFontFamily>(
                GetProcAddress(module, s!("GdipDeleteFontFamily")).ok_or(())?,
            ),
            create_font: std::mem::transmute::<RawProc, FnGdipCreateFont>(
                GetProcAddress(module, s!("GdipCreateFont")).ok_or(())?,
            ),
            delete_font: std::mem::transmute::<RawProc, FnGdipDeleteFont>(
                GetProcAddress(module, s!("GdipDeleteFont")).ok_or(())?,
            ),
        })
        .map_err(|_| ())?;
    let detour =
        GenericDetour::<FnGdipDrawString>::new(target, gdip_draw_string_detour).map_err(|_| ())?;
    HOOK.set(detour).map_err(|_| ())?;
    HOOK.get().ok_or(())?.enable().map_err(|_| ())
}

#[no_mangle]
pub extern "C" fn glyphshift_adapter_entry_v1() -> NativeAdapterApiV1 {
    NativeAdapterApiV1 {
        struct_size: std::mem::size_of::<NativeAdapterApiV1>() as u32,
        descriptor: NativeAdapterDescriptorV1::inline_target(
            ADAPTER_ID,
            (1, 0, 0),
            SUPPORTED_FEATURES,
            PLATFORM_WINDOWS,
            ARCH_X86 | ARCH_X86_64,
        ),
        negotiate_features,
        activate,
        deactivate,
    }
}

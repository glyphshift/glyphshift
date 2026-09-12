//! Native DirectWrite `CreateTextLayout` + Direct2D `DrawTextLayout` Adapter package.

mod uniform_layout;

use glyphshift_adapter_directwrite::ADAPTER_ID;
use glyphshift_adapter_native_abi::{
    DecideUtf16V1, NativeAdapterApiV1, NativeAdapterDescriptorV1, NativeDecisionV1,
    NativeNegotiationV1, NativeRuntimeHostV1, ARCH_X86, ARCH_X86_64, DECISION_TEXT_REPLACE,
    FEATURE_TEXT_OBSERVE, FEATURE_TEXT_REPLACE, PLATFORM_WINDOWS, STATUS_ACTIVATION_FAILED,
    STATUS_INVALID_HOST, STATUS_OK, STATUS_UNAUTHORIZED_FEATURE, STATUS_UNSUPPORTED_FEATURE,
};
use glyphshift_adapter_native_abi::{NativeTextEventV1, NativeTextHostBinding, NativeTextHostV1};
use retour::GenericDetour;
use std::cell::Cell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{OnceLock, RwLock};
use windows::core::{Interface, HRESULT, PCWSTR};
use windows::Win32::Foundation::BOOL;
use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_ALPHA_MODE_IGNORE, D2D1_PIXEL_FORMAT, D2D_POINT_2F,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1CreateFactory, ID2D1Factory, ID2D1RenderTarget, D2D1_COMPATIBLE_RENDER_TARGET_OPTIONS_NONE,
    D2D1_DRAW_TEXT_OPTIONS, D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_FEATURE_LEVEL_DEFAULT,
    D2D1_RENDER_TARGET_PROPERTIES, D2D1_RENDER_TARGET_TYPE_DEFAULT, D2D1_RENDER_TARGET_USAGE_NONE,
};
use windows::Win32::Graphics::DirectWrite::{
    DWriteCreateFactory, IDWriteFactory, IDWriteFontCollection, IDWriteInlineObject,
    IDWriteTextLayout, IDWriteTypography, DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL,
    DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT_NORMAL, DWRITE_TEXT_RANGE,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;

const SUPPORTED_FEATURES: u64 = FEATURE_TEXT_OBSERVE | FEATURE_TEXT_REPLACE;
const MAX_TEXT_UNITS: usize = 16 * 1024;
const MAX_LAYOUTS: usize = 4 * 1024;
const MAX_FONT_UNITS: usize = 63;

type FnCreateTextLayout = unsafe extern "system" fn(
    *mut core::ffi::c_void,
    PCWSTR,
    u32,
    *mut core::ffi::c_void,
    f32,
    f32,
    *mut *mut core::ffi::c_void,
) -> HRESULT;

type FnDrawTextLayout = unsafe extern "system" fn(
    *mut core::ffi::c_void,
    D2D_POINT_2F,
    *mut core::ffi::c_void,
    *mut core::ffi::c_void,
    D2D1_DRAW_TEXT_OPTIONS,
);

#[derive(Clone, Copy)]
struct HostBridge {
    context: usize,
    decide_utf16: DecideUtf16V1,
    text_host: Option<NativeTextHostBinding>,
}

#[derive(Clone)]
struct LayoutRecord {
    source: Box<str>,
    sequence: u64,
}

struct DecisionBuffers {
    decision: NativeDecisionV1,
    text: Vec<u16>,
}

static ACTIVE_FEATURES: AtomicU64 = AtomicU64::new(0);
static NEXT_LAYOUT_SEQUENCE: AtomicU64 = AtomicU64::new(1);
static HOST: OnceLock<RwLock<HostBridge>> = OnceLock::new();
static LAYOUTS: OnceLock<RwLock<HashMap<usize, LayoutRecord>>> = OnceLock::new();
static CREATE_HOOK: OnceLock<GenericDetour<FnCreateTextLayout>> = OnceLock::new();
static DRAW_PRIMARY_HOOK: OnceLock<GenericDetour<FnDrawTextLayout>> = OnceLock::new();
static DRAW_BITMAP_HOOK: OnceLock<GenericDetour<FnDrawTextLayout>> = OnceLock::new();

thread_local! {
    static IN_CALLBACK: Cell<bool> = const { Cell::new(false) };
}

struct CallbackGuard;

impl CallbackGuard {
    fn enter() -> Option<Self> {
        IN_CALLBACK.with(|active| (!active.replace(true)).then(|| Self))
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
        text_host: TEXT_HOST
            .lock()
            .ok()
            .and_then(|binding| *binding)
            .filter(|binding| binding.matches(&host)),
    };
    let host_ready = if let Some(current) = HOST.get() {
        current.write().map(|mut current| *current = bridge).is_ok()
    } else {
        HOST.set(RwLock::new(bridge)).is_ok()
    };
    if !host_ready || unsafe { install_hooks() }.is_err() {
        return negotiation_error(STATUS_ACTIVATION_FAILED);
    }
    ACTIVE_FEATURES.store(negotiated.active_feature_bits, Ordering::Release);
    negotiated
}

extern "C" fn deactivate() -> i32 {
    ACTIVE_FEATURES.store(0, Ordering::Release);
    if let Some(layouts) = LAYOUTS.get() {
        if let Ok(mut layouts) = layouts.write() {
            layouts.clear();
        }
    }
    STATUS_OK
}

unsafe fn read_text(text: PCWSTR, length: u32) -> Option<Box<str>> {
    let length = usize::try_from(length).ok()?;
    if text.is_null() || length == 0 || length > MAX_TEXT_UNITS {
        return None;
    }
    String::from_utf16(std::slice::from_raw_parts(text.0, length))
        .ok()
        .map(Into::into)
}

fn callback_active() -> bool {
    IN_CALLBACK.with(Cell::get)
}

fn remember_layout(layout: usize, mut record: LayoutRecord) {
    let layouts = LAYOUTS.get_or_init(|| RwLock::new(HashMap::new()));
    if let Ok(mut layouts) = layouts.write() {
        if layouts.len() >= MAX_LAYOUTS {
            let cutoff = layouts
                .values()
                .map(|record| record.sequence)
                .min()
                .unwrap_or_default()
                .saturating_add((MAX_LAYOUTS / 4) as u64);
            layouts.retain(|_, record| record.sequence >= cutoff);
        }
        record.sequence = NEXT_LAYOUT_SEQUENCE.fetch_add(1, Ordering::AcqRel);
        layouts.insert(layout, record);
    }
}

fn observed_layout(layout: usize) -> Option<LayoutRecord> {
    LAYOUTS.get()?.read().ok()?.get(&layout).cloned()
}

fn decide(source: &str) -> Option<DecisionBuffers> {
    let host = *HOST.get()?.read().ok()?;
    let source = source.encode_utf16().collect::<Vec<_>>();
    let mut text = vec![0_u16; MAX_TEXT_UNITS];
    let mut font = vec![0_u16; MAX_FONT_UNITS];
    let decision = if let Some(extended) = host.text_host {
        extended.decide(
            &NativeTextEventV1::complete_draw(&source),
            &mut text,
            &mut font,
        )
    } else {
        (host.decide_utf16)(
            host.context as *mut core::ffi::c_void,
            source.as_ptr(),
            source.len() as u32,
            text.as_mut_ptr(),
            text.len() as u32,
            font.as_mut_ptr(),
            font.len() as u32,
        )
    };

    if decision.status != STATUS_OK || decision.text_len as usize > text.len() {
        return None;
    }
    text.truncate(decision.text_len as usize);
    Some(DecisionBuffers { decision, text })
}

fn range_covers_layout(range: DWRITE_TEXT_RANGE, length: u32) -> bool {
    range.startPosition == 0 && range.length >= length
}

unsafe fn layout_is_uniform(layout: &IDWriteTextLayout, length: u32) -> bool {
    if length == 0 {
        return false;
    }
    let mut range = DWRITE_TEXT_RANGE::default();
    let mut family_length = 0_u32;
    if layout
        .GetFontFamilyNameLength(0, &mut family_length, Some(&mut range))
        .is_err()
        || !range_covers_layout(range, length)
    {
        return false;
    }
    let mut collection: Option<IDWriteFontCollection> = None;
    range = DWRITE_TEXT_RANGE::default();
    if layout
        .GetFontCollection(0, &mut collection, Some(&mut range))
        .is_err()
        || !range_covers_layout(range, length)
    {
        return false;
    }
    let mut weight = DWRITE_FONT_WEIGHT_NORMAL;
    range = DWRITE_TEXT_RANGE::default();
    if layout
        .GetFontWeight(0, &mut weight, Some(&mut range))
        .is_err()
        || !range_covers_layout(range, length)
    {
        return false;
    }
    let mut style = DWRITE_FONT_STYLE_NORMAL;
    range = DWRITE_TEXT_RANGE::default();
    if layout
        .GetFontStyle(0, &mut style, Some(&mut range))
        .is_err()
        || !range_covers_layout(range, length)
    {
        return false;
    }
    let mut stretch = DWRITE_FONT_STRETCH_NORMAL;
    range = DWRITE_TEXT_RANGE::default();
    if layout
        .GetFontStretch(0, &mut stretch, Some(&mut range))
        .is_err()
        || !range_covers_layout(range, length)
    {
        return false;
    }
    let mut size = 0.0_f32;
    range = DWRITE_TEXT_RANGE::default();
    if layout.GetFontSize(0, &mut size, Some(&mut range)).is_err()
        || !range_covers_layout(range, length)
    {
        return false;
    }
    let mut locale_length = 0_u32;
    range = DWRITE_TEXT_RANGE::default();
    if layout
        .GetLocaleNameLength(0, &mut locale_length, Some(&mut range))
        .is_err()
        || !range_covers_layout(range, length)
    {
        return false;
    }
    let mut underline = BOOL::default();
    range = DWRITE_TEXT_RANGE::default();
    if layout
        .GetUnderline(0, &mut underline, Some(&mut range))
        .is_err()
        || !range_covers_layout(range, length)
    {
        return false;
    }
    let mut strikethrough = BOOL::default();
    range = DWRITE_TEXT_RANGE::default();
    if layout
        .GetStrikethrough(0, &mut strikethrough, Some(&mut range))
        .is_err()
        || !range_covers_layout(range, length)
    {
        return false;
    }
    let mut effect = None;
    range = DWRITE_TEXT_RANGE::default();
    if layout
        .GetDrawingEffect(0, &mut effect, Some(&mut range))
        .is_err()
        || effect.is_some()
        || !range_covers_layout(range, length)
    {
        return false;
    }
    let mut inline: Option<IDWriteInlineObject> = None;
    range = DWRITE_TEXT_RANGE::default();
    if layout
        .GetInlineObject(0, &mut inline, Some(&mut range))
        .is_err()
        || inline.is_some()
        || !range_covers_layout(range, length)
    {
        return false;
    }
    let mut typography: Option<IDWriteTypography> = None;
    range = DWRITE_TEXT_RANGE::default();
    layout
        .GetTypography(0, &mut typography, Some(&mut range))
        .is_ok()
        && range_covers_layout(range, length)
        && uniform_layout::extended_is_uniform(layout, length)
}

unsafe extern "system" fn create_text_layout_detour(
    factory: *mut core::ffi::c_void,
    text: PCWSTR,
    length: u32,
    format: *mut core::ffi::c_void,
    max_width: f32,
    max_height: f32,
    layout_out: *mut *mut core::ffi::c_void,
) -> HRESULT {
    let Some(original) = CREATE_HOOK.get() else {
        return HRESULT::from_win32(1);
    };
    let captured = if ACTIVE_FEATURES.load(Ordering::Acquire) != 0 && !callback_active() {
        read_text(text, length)
    } else {
        None
    };
    let result = original.call(
        factory, text, length, format, max_width, max_height, layout_out,
    );
    if result.is_ok() && !layout_out.is_null() {
        let layout = *layout_out;
        if !layout.is_null() {
            if let Some(source) = captured {
                remember_layout(
                    layout as usize,
                    LayoutRecord {
                        source,
                        sequence: 0,
                    },
                );
            } else if let Some(layouts) = LAYOUTS.get() {
                // An empty or unobserved creation may reuse a released layout's
                // address. It must never inherit that object's former source.
                if let Ok(mut layouts) = layouts.write() {
                    layouts.remove(&(layout as usize));
                }
            }
        }
    }
    result
}

fn replacement_layout(layout: *mut core::ffi::c_void) -> Option<IDWriteTextLayout> {
    let active = ACTIVE_FEATURES.load(Ordering::Acquire);
    if active == 0 {
        return None;
    }
    let _guard = CallbackGuard::enter()?;
    let record = observed_layout(layout as usize)?;
    // A known draw remains observable when its formatting cannot be replaced safely.
    let decision = std::panic::catch_unwind(|| decide(&record.source))
        .ok()
        .flatten()?;
    if active & FEATURE_TEXT_REPLACE == 0
        || decision.decision.decision_bits & DECISION_TEXT_REPLACE == 0
        || decision.text.is_empty()
    {
        return None;
    }
    let layout_ref = unsafe { IDWriteTextLayout::from_raw_borrowed(&layout) }?;
    if !unsafe { layout_is_uniform(layout_ref, record.source.encode_utf16().count() as u32) } {
        return None;
    }
    let factory: IDWriteFactory =
        unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED) }.ok()?;
    unsafe {
        let replacement = factory
            .CreateTextLayout(
                &decision.text,
                layout_ref,
                layout_ref.GetMaxWidth(),
                layout_ref.GetMaxHeight(),
            )
            .ok()?;
        uniform_layout::copy(layout_ref, &replacement, decision.text.len() as u32).ok()?;
        Some(replacement)
    }
}

unsafe extern "system" fn draw_text_layout_primary_detour(
    render_target: *mut core::ffi::c_void,
    origin: D2D_POINT_2F,
    layout: *mut core::ffi::c_void,
    brush: *mut core::ffi::c_void,
    options: D2D1_DRAW_TEXT_OPTIONS,
) {
    let Some(original) = DRAW_PRIMARY_HOOK.get() else {
        return;
    };
    let scope = layout_scope(layout);
    if let Some(replacement) = replacement_layout(layout) {
        original.call(
            render_target,
            origin,
            Interface::as_raw(&replacement),
            brush,
            options,
        );
    } else {
        drop(scope);
        original.call(render_target, origin, layout, brush, options);
    }
}

unsafe extern "system" fn draw_text_layout_bitmap_detour(
    render_target: *mut core::ffi::c_void,
    origin: D2D_POINT_2F,
    layout: *mut core::ffi::c_void,
    brush: *mut core::ffi::c_void,
    options: D2D1_DRAW_TEXT_OPTIONS,
) {
    let Some(original) = DRAW_BITMAP_HOOK.get() else {
        return;
    };
    let scope = layout_scope(layout);
    if let Some(replacement) = replacement_layout(layout) {
        original.call(
            render_target,
            origin,
            Interface::as_raw(&replacement),
            brush,
            options,
        );
    } else {
        drop(scope);
        original.call(render_target, origin, layout, brush, options);
    }
}

unsafe fn install_hooks() -> Result<(), ()> {
    if CREATE_HOOK.get().is_none() {
        let factory: IDWriteFactory =
            DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED).map_err(|_| ())?;
        let target = Interface::vtable(&factory).CreateTextLayout;
        let detour = GenericDetour::<FnCreateTextLayout>::new(target, create_text_layout_detour)
            .map_err(|_| ())?;
        CREATE_HOOK.set(detour).map_err(|_| ())?;
        CREATE_HOOK.get().ok_or(())?.enable().map_err(|_| ())?;
    }
    if DRAW_PRIMARY_HOOK.get().is_none() {
        let factory: ID2D1Factory =
            D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None).map_err(|_| ())?;
        let properties = D2D1_RENDER_TARGET_PROPERTIES {
            r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_IGNORE,
            },
            dpiX: 0.0,
            dpiY: 0.0,
            usage: D2D1_RENDER_TARGET_USAGE_NONE,
            minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
        };
        let target = factory.CreateDCRenderTarget(&properties).map_err(|_| ())?;
        let render_target: ID2D1RenderTarget = target.cast().map_err(|_| ())?;
        let primary_target = Interface::vtable(&render_target).DrawTextLayout;
        let bitmap = render_target
            .CreateCompatibleRenderTarget(
                None,
                None,
                None,
                D2D1_COMPATIBLE_RENDER_TARGET_OPTIONS_NONE,
            )
            .map_err(|_| ())?;
        let bitmap_target: ID2D1RenderTarget = bitmap.cast().map_err(|_| ())?;
        let bitmap_target = Interface::vtable(&bitmap_target).DrawTextLayout;

        let detour =
            GenericDetour::<FnDrawTextLayout>::new(primary_target, draw_text_layout_primary_detour)
                .map_err(|_| ())?;
        DRAW_PRIMARY_HOOK.set(detour).map_err(|_| ())?;
        DRAW_PRIMARY_HOOK
            .get()
            .ok_or(())?
            .enable()
            .map_err(|_| ())?;

        if bitmap_target as usize != primary_target as usize {
            let detour = GenericDetour::<FnDrawTextLayout>::new(
                bitmap_target,
                draw_text_layout_bitmap_detour,
            )
            .map_err(|_| ())?;
            DRAW_BITMAP_HOOK.set(detour).map_err(|_| ())?;
            DRAW_BITMAP_HOOK.get().ok_or(())?.enable().map_err(|_| ())?;
        }
    }
    Ok(())
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
        request_refresh: glyphshift_adapter_native_abi::request_refresh_noop,
    }
}

static TEXT_HOST: std::sync::Mutex<Option<NativeTextHostBinding>> = std::sync::Mutex::new(None);
/// Optional source-evidence binding; the V1 host and legacy rendering path stay valid.
///
/// # Safety
/// A non-null host must point to a readable V1 extension whose context and callbacks
/// remain valid until every Adapter callback has finished.
#[no_mangle]
pub unsafe extern "C" fn glyphshift_adapter_bind_text_host_v1(
    host: *const NativeTextHostV1,
) -> i32 {
    let value = if host.is_null() {
        None
    } else {
        let Some(value) = NativeTextHostBinding::new(unsafe { *host }) else {
            return STATUS_INVALID_HOST;
        };
        Some(value)
    };
    match TEXT_HOST.lock() {
        Ok(mut binding) => {
            *binding = value;
            STATUS_OK
        }
        Err(_) => STATUS_INVALID_HOST,
    }
}

fn text_scope() -> Option<glyphshift_adapter_native_abi::NativeTextScope> {
    let binding = HOST.get()?.read().ok()?.text_host?;
    binding.enter_scope()
}

fn layout_scope(
    layout: *mut core::ffi::c_void,
) -> Option<glyphshift_adapter_native_abi::NativeTextScope> {
    let active = ACTIVE_FEATURES.load(Ordering::Acquire);
    if active == 0 || callback_active() {
        return None;
    }
    let record = observed_layout(layout as usize)?;
    if active & FEATURE_TEXT_REPLACE != 0 {
        let value = unsafe { IDWriteTextLayout::from_raw_borrowed(&layout) }?;
        if !unsafe { layout_is_uniform(value, record.source.encode_utf16().count() as u32) } {
            return None;
        }
    }
    text_scope()
}

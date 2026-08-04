//! Native `ID2D1RenderTarget::DrawText` package for the Direct2D Adapter.

use glyphshift_adapter_direct2d::ADAPTER_ID;
use glyphshift_adapter_native_abi::{
    DecideUtf16V1, NativeAdapterApiV1, NativeAdapterDescriptorV1, NativeDecisionV1,
    NativeNegotiationV1, NativeRuntimeHostV1, ARCH_X86, ARCH_X86_64, DECISION_TEXT_REPLACE,
    FEATURE_TEXT_OBSERVE, FEATURE_TEXT_REPLACE, PLATFORM_WINDOWS, STATUS_ACTIVATION_FAILED,
    STATUS_INVALID_HOST, STATUS_OK, STATUS_UNAUTHORIZED_FEATURE, STATUS_UNSUPPORTED_FEATURE,
};
use retour::GenericDetour;
use std::cell::Cell;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{OnceLock, RwLock};
use windows::core::{Interface, PCWSTR};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_ALPHA_MODE_IGNORE, D2D1_PIXEL_FORMAT, D2D_RECT_F,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1CreateFactory, ID2D1Factory, ID2D1RenderTarget, D2D1_DRAW_TEXT_OPTIONS,
    D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_FEATURE_LEVEL_DEFAULT, D2D1_RENDER_TARGET_PROPERTIES,
    D2D1_RENDER_TARGET_TYPE_DEFAULT, D2D1_RENDER_TARGET_USAGE_NONE,
};
use windows::Win32::Graphics::DirectWrite::DWRITE_MEASURING_MODE;
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;

const SUPPORTED_FEATURES: u64 = FEATURE_TEXT_OBSERVE | FEATURE_TEXT_REPLACE;
const MAX_TEXT_UNITS: usize = 16 * 1024;
const MAX_FONT_UNITS: usize = 63;

type FnDrawText = unsafe extern "system" fn(
    *mut core::ffi::c_void,
    PCWSTR,
    u32,
    *mut core::ffi::c_void,
    *const D2D_RECT_F,
    *mut core::ffi::c_void,
    D2D1_DRAW_TEXT_OPTIONS,
    DWRITE_MEASURING_MODE,
);

#[derive(Clone, Copy)]
struct HostBridge {
    context: usize,
    decide_utf16: DecideUtf16V1,
}

struct DecisionBuffers {
    decision: NativeDecisionV1,
    text: Vec<u16>,
}

static ACTIVE_FEATURES: AtomicU64 = AtomicU64::new(0);
static HOST: OnceLock<RwLock<HostBridge>> = OnceLock::new();
static HOOK: OnceLock<GenericDetour<FnDrawText>> = OnceLock::new();

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
    Some(DecisionBuffers { decision, text })
}

unsafe fn read_text(text: PCWSTR, length: u32) -> Option<String> {
    let length = usize::try_from(length).ok()?;
    if text.is_null() || length == 0 || length > MAX_TEXT_UNITS {
        return None;
    }
    String::from_utf16(std::slice::from_raw_parts(text.0, length)).ok()
}

unsafe extern "system" fn draw_text_detour(
    render_target: *mut core::ffi::c_void,
    text: PCWSTR,
    length: u32,
    format: *mut core::ffi::c_void,
    layout: *const D2D_RECT_F,
    brush: *mut core::ffi::c_void,
    options: D2D1_DRAW_TEXT_OPTIONS,
    measuring_mode: DWRITE_MEASURING_MODE,
) {
    let Some(original) = HOOK.get() else {
        return;
    };
    if ACTIVE_FEATURES.load(Ordering::Acquire) == 0 {
        return original.call(
            render_target,
            text,
            length,
            format,
            layout,
            brush,
            options,
            measuring_mode,
        );
    }
    let Some(_guard) = CallbackGuard::enter() else {
        return original.call(
            render_target,
            text,
            length,
            format,
            layout,
            brush,
            options,
            measuring_mode,
        );
    };
    let decision = read_text(text, length)
        .and_then(|source| std::panic::catch_unwind(|| decide(&source)).ok().flatten());
    let Some(decision) = decision else {
        return original.call(
            render_target,
            text,
            length,
            format,
            layout,
            brush,
            options,
            measuring_mode,
        );
    };
    let replace = ACTIVE_FEATURES.load(Ordering::Acquire) & FEATURE_TEXT_REPLACE != 0
        && decision.decision.decision_bits & DECISION_TEXT_REPLACE != 0;
    if replace {
        original.call(
            render_target,
            PCWSTR(decision.text.as_ptr()),
            decision.text.len() as u32,
            format,
            layout,
            brush,
            options,
            measuring_mode,
        );
    } else {
        original.call(
            render_target,
            text,
            length,
            format,
            layout,
            brush,
            options,
            measuring_mode,
        );
    }
}

unsafe fn install_hook() -> Result<(), ()> {
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
    let draw_text = Interface::vtable(&render_target).DrawText;
    let detour = GenericDetour::<FnDrawText>::new(draw_text, draw_text_detour).map_err(|_| ())?;
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

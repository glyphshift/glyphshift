//! Native `TextOutW` package for the first-party GDI Adapter family.

use glyphshift_adapter_gdi::TEXT_OUT_ADAPTER_ID;
use glyphshift_adapter_gdi_native_support::{
    commit_activation, decide, is_active, prepare_activation, read_text, with_replacement_font,
    CallbackGuard, SUPPORTED_FEATURES,
};
use glyphshift_adapter_native_abi::{
    NativeAdapterApiV1, NativeAdapterDescriptorV1, NativeNegotiationV1, NativeRuntimeHostV1,
    ARCH_X86, ARCH_X86_64, PLATFORM_WINDOWS, STATUS_ACTIVATION_FAILED, STATUS_OK,
};
use retour::GenericDetour;
use std::sync::OnceLock;
use windows::core::{s, w};
use windows::Win32::Foundation::BOOL;
use windows::Win32::Graphics::Gdi::HDC;
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};

type FnTextOutW = unsafe extern "system" fn(HDC, i32, i32, *const u16, i32) -> BOOL;

static HOOK: OnceLock<GenericDetour<FnTextOutW>> = OnceLock::new();

extern "C" fn activate(
    host: *const NativeRuntimeHostV1,
    requested: u64,
    granted: u64,
) -> NativeNegotiationV1 {
    let negotiated = prepare_activation(host, requested, granted);
    if negotiated.status != STATUS_OK {
        return negotiated;
    }
    if HOOK.get().is_none() && unsafe { install_hook() }.is_err() {
        return NativeNegotiationV1 {
            status: STATUS_ACTIVATION_FAILED,
            active_feature_bits: 0,
        };
    }
    commit_activation(negotiated.active_feature_bits);
    negotiated
}

unsafe extern "system" fn text_out_w_detour(
    hdc: HDC,
    x: i32,
    y: i32,
    text: *const u16,
    count: i32,
) -> BOOL {
    let Some(original) = HOOK.get() else {
        return BOOL(0);
    };
    if !is_active() || count <= 0 {
        return original.call(hdc, x, y, text, count);
    }
    let Some(_guard) = CallbackGuard::enter() else {
        return original.call(hdc, x, y, text, count);
    };
    let Some(source) = read_text(text, count) else {
        return original.call(hdc, x, y, text, count);
    };
    let decision = std::panic::catch_unwind(|| decide(source.trim()))
        .ok()
        .flatten();
    let Some(decision) = decision else {
        return original.call(hdc, x, y, text, count);
    };
    let (draw_text, draw_count) = decision
        .replacement_text()
        .map_or((text, count), |replacement| {
            (replacement.as_ptr(), replacement.len() as i32)
        });
    with_replacement_font(hdc, &decision, || {
        original.call(hdc, x, y, draw_text, draw_count)
    })
}

unsafe fn install_hook() -> Result<(), ()> {
    let module = GetModuleHandleW(w!("gdi32.dll")).map_err(|_| ())?;
    let address = GetProcAddress(module, s!("TextOutW")).ok_or(())?;
    let target: FnTextOutW = std::mem::transmute(address);
    let detour = GenericDetour::<FnTextOutW>::new(target, text_out_w_detour).map_err(|_| ())?;
    HOOK.set(detour).map_err(|_| ())?;
    HOOK.get().ok_or(())?.enable().map_err(|_| ())
}

#[no_mangle]
pub extern "C" fn glyphshift_adapter_entry_v1() -> NativeAdapterApiV1 {
    NativeAdapterApiV1 {
        struct_size: std::mem::size_of::<NativeAdapterApiV1>() as u32,
        descriptor: NativeAdapterDescriptorV1::inline_target(
            TEXT_OUT_ADAPTER_ID,
            (1, 0, 0),
            SUPPORTED_FEATURES,
            PLATFORM_WINDOWS,
            ARCH_X86 | ARCH_X86_64,
        ),
        negotiate_features: glyphshift_adapter_gdi_native_support::negotiate_features,
        activate,
        deactivate: glyphshift_adapter_gdi_native_support::deactivate,
    }
}

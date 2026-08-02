//! Native `DrawTextW` and `DrawTextExW` package for the Win32 USER text API family.

use glyphshift_adapter_gdi::DRAW_TEXT_ADAPTER_ID;
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
use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Gdi::HDC;
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};

const DT_MODIFYSTRING: u32 = 0x0001_0000;

type FnDrawTextW = unsafe extern "system" fn(HDC, *const u16, i32, *mut RECT, u32) -> i32;
type FnDrawTextExW =
    unsafe extern "system" fn(HDC, *mut u16, i32, *mut RECT, u32, *mut core::ffi::c_void) -> i32;

static DRAW_TEXT_HOOK: OnceLock<GenericDetour<FnDrawTextW>> = OnceLock::new();
static DRAW_TEXT_EX_HOOK: OnceLock<GenericDetour<FnDrawTextExW>> = OnceLock::new();

extern "C" fn activate(
    host: *const NativeRuntimeHostV1,
    requested: u64,
    granted: u64,
) -> NativeNegotiationV1 {
    let negotiated = prepare_activation(host, requested, granted);
    if negotiated.status != STATUS_OK {
        return negotiated;
    }
    if unsafe { install_hooks() }.is_err() {
        return NativeNegotiationV1 {
            status: STATUS_ACTIVATION_FAILED,
            active_feature_bits: 0,
        };
    }
    commit_activation(negotiated.active_feature_bits);
    negotiated
}

unsafe extern "system" fn draw_text_w_detour(
    hdc: HDC,
    text: *const u16,
    count: i32,
    rect: *mut RECT,
    format: u32,
) -> i32 {
    let Some(original) = DRAW_TEXT_HOOK.get() else {
        return 0;
    };
    if !is_active() {
        return original.call(hdc, text, count, rect, format);
    }
    let Some(_guard) = CallbackGuard::enter() else {
        return original.call(hdc, text, count, rect, format);
    };
    let Some(source) = read_text(text, count) else {
        return original.call(hdc, text, count, rect, format);
    };
    let decision = std::panic::catch_unwind(|| decide(source.trim()))
        .ok()
        .flatten();
    let Some(decision) = decision else {
        return original.call(hdc, text, count, rect, format);
    };
    let (draw_text, draw_count) = decision
        .replacement_text()
        .map_or((text, count), |replacement| {
            (replacement.as_ptr(), replacement.len() as i32)
        });
    with_replacement_font(hdc, &decision, || {
        original.call(hdc, draw_text, draw_count, rect, format)
    })
}

unsafe extern "system" fn draw_text_ex_w_detour(
    hdc: HDC,
    text: *mut u16,
    count: i32,
    rect: *mut RECT,
    format: u32,
    params: *mut core::ffi::c_void,
) -> i32 {
    let Some(original) = DRAW_TEXT_EX_HOOK.get() else {
        return 0;
    };
    if !is_active() {
        return original.call(hdc, text, count, rect, format, params);
    }
    let Some(_guard) = CallbackGuard::enter() else {
        return original.call(hdc, text, count, rect, format, params);
    };
    let Some(source) = read_text(text, count) else {
        return original.call(hdc, text, count, rect, format, params);
    };
    let decision = std::panic::catch_unwind(|| decide(source.trim()))
        .ok()
        .flatten();
    let Some(decision) = decision else {
        return original.call(hdc, text, count, rect, format, params);
    };
    let replacement = (format & DT_MODIFYSTRING == 0)
        .then(|| decision.replacement_text())
        .flatten();
    let (draw_text, draw_count) = replacement.map_or((text, count), |replacement| {
        (replacement.as_ptr().cast_mut(), replacement.len() as i32)
    });
    with_replacement_font(hdc, &decision, || {
        original.call(hdc, draw_text, draw_count, rect, format, params)
    })
}

unsafe fn install_hooks() -> Result<(), ()> {
    let module = GetModuleHandleW(w!("user32.dll")).map_err(|_| ())?;
    if DRAW_TEXT_HOOK.get().is_none() {
        let address = GetProcAddress(module, s!("DrawTextW")).ok_or(())?;
        let target: FnDrawTextW = std::mem::transmute(address);
        let detour =
            GenericDetour::<FnDrawTextW>::new(target, draw_text_w_detour).map_err(|_| ())?;
        DRAW_TEXT_HOOK.set(detour).map_err(|_| ())?;
        DRAW_TEXT_HOOK.get().ok_or(())?.enable().map_err(|_| ())?;
    }
    if DRAW_TEXT_EX_HOOK.get().is_none() {
        let address = GetProcAddress(module, s!("DrawTextExW")).ok_or(())?;
        let target: FnDrawTextExW = std::mem::transmute(address);
        let detour =
            GenericDetour::<FnDrawTextExW>::new(target, draw_text_ex_w_detour).map_err(|_| ())?;
        DRAW_TEXT_EX_HOOK.set(detour).map_err(|_| ())?;
        DRAW_TEXT_EX_HOOK
            .get()
            .ok_or(())?
            .enable()
            .map_err(|_| ())?;
    }
    Ok(())
}

#[no_mangle]
pub extern "C" fn glyphshift_adapter_entry_v1() -> NativeAdapterApiV1 {
    NativeAdapterApiV1 {
        struct_size: std::mem::size_of::<NativeAdapterApiV1>() as u32,
        descriptor: NativeAdapterDescriptorV1::inline_target(
            DRAW_TEXT_ADAPTER_ID,
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

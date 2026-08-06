//! Native observe-only `WriteConsoleW` package for Console clients.

use glyphshift_adapter_console::{visible_console_text, ADAPTER_ID};
use glyphshift_adapter_native_abi::{
    DecideUtf16V1, NativeAdapterApiV1, NativeAdapterDescriptorV1, NativeNegotiationV1,
    NativeRuntimeHostV1, ARCH_X86, ARCH_X86_64, FEATURE_TEXT_OBSERVE, PLATFORM_WINDOWS,
    STATUS_ACTIVATION_FAILED, STATUS_INVALID_HOST, STATUS_OK, STATUS_UNAUTHORIZED_FEATURE,
    STATUS_UNSUPPORTED_FEATURE,
};
use retour::GenericDetour;
use std::cell::Cell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{OnceLock, RwLock};
use windows::core::{s, w};
use windows::Win32::Foundation::{BOOL, HANDLE};
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};

const MAX_TEXT_UNITS: usize = 16 * 1024;

type FnWriteConsoleW = unsafe extern "system" fn(
    HANDLE,
    *const core::ffi::c_void,
    u32,
    *mut u32,
    *const core::ffi::c_void,
) -> BOOL;

#[derive(Clone, Copy)]
struct HostBridge {
    context: usize,
    decide_utf16: DecideUtf16V1,
}

static ACTIVE: AtomicBool = AtomicBool::new(false);
static HOST: OnceLock<RwLock<HostBridge>> = OnceLock::new();
static HOOK: OnceLock<GenericDetour<FnWriteConsoleW>> = OnceLock::new();

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
    if requested & !FEATURE_TEXT_OBSERVE != 0 {
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
    if !host_ready || (HOOK.get().is_none() && unsafe { install_hook() }.is_err()) {
        return negotiation_error(STATUS_ACTIVATION_FAILED);
    }
    ACTIVE.store(
        negotiated.active_feature_bits & FEATURE_TEXT_OBSERVE != 0,
        Ordering::Release,
    );
    negotiated
}

extern "C" fn deactivate() -> i32 {
    ACTIVE.store(false, Ordering::Release);
    STATUS_OK
}

unsafe fn read_text(buffer: *const core::ffi::c_void, length: u32) -> Option<String> {
    let length = usize::try_from(length).ok()?;
    if buffer.is_null() || length == 0 || length > MAX_TEXT_UNITS {
        return None;
    }
    String::from_utf16(std::slice::from_raw_parts(buffer.cast::<u16>(), length)).ok()
}

fn observe(source: &str) {
    let Some(host) = HOST
        .get()
        .and_then(|host| host.read().ok())
        .map(|host| *host)
    else {
        return;
    };
    let source = source.encode_utf16().collect::<Vec<_>>();
    let mut text = vec![0_u16; MAX_TEXT_UNITS];
    let mut font = vec![0_u16; 63];
    let _ = (host.decide_utf16)(
        host.context as *mut core::ffi::c_void,
        source.as_ptr(),
        source.len() as u32,
        text.as_mut_ptr(),
        text.len() as u32,
        font.as_mut_ptr(),
        font.len() as u32,
    );
}

unsafe extern "system" fn write_console_w_detour(
    output: HANDLE,
    buffer: *const core::ffi::c_void,
    length: u32,
    written: *mut u32,
    reserved: *const core::ffi::c_void,
) -> BOOL {
    let Some(original) = HOOK.get() else {
        return BOOL(0);
    };
    if ACTIVE.load(Ordering::Acquire) {
        if let Some(_guard) = CallbackGuard::enter() {
            if let Some(source) = read_text(buffer, length) {
                let visible = visible_console_text(&source);
                for segment in visible
                    .split(['\r', '\n'])
                    .map(str::trim)
                    .filter(|segment| !segment.is_empty())
                {
                    let _ = std::panic::catch_unwind(|| observe(segment));
                }
            }
        }
    }
    original.call(output, buffer, length, written, reserved)
}

unsafe fn install_hook() -> Result<(), ()> {
    let module = GetModuleHandleW(w!("kernelbase.dll")).map_err(|_| ())?;
    let address = GetProcAddress(module, s!("WriteConsoleW")).ok_or(())?;
    let target: FnWriteConsoleW = std::mem::transmute(address);
    let detour =
        GenericDetour::<FnWriteConsoleW>::new(target, write_console_w_detour).map_err(|_| ())?;
    HOOK.set(detour).map_err(|_| ())?;
    HOOK.get().ok_or(())?.enable().map_err(|_| ())
}

#[no_mangle]
pub extern "C" fn glyphshift_adapter_entry_v1() -> NativeAdapterApiV1 {
    NativeAdapterApiV1 {
        struct_size: std::mem::size_of::<NativeAdapterApiV1>() as u32,
        descriptor: NativeAdapterDescriptorV1::observe_target(
            ADAPTER_ID,
            (1, 0, 0),
            FEATURE_TEXT_OBSERVE,
            PLATFORM_WINDOWS,
            ARCH_X86 | ARCH_X86_64,
        ),
        negotiate_features,
        activate,
        deactivate,
    }
}

//! Shared fail-open host bridge and font substitution for Win32 GDI text seams.

use glyphshift_adapter_native_abi::{
    DecideUtf16V1, NativeDecisionV1, NativeNegotiationV1, NativeRuntimeHostV1,
    DECISION_FONT_SUBSTITUTE, DECISION_TEXT_REPLACE, FEATURE_FONT_SUBSTITUTE, FEATURE_TEXT_OBSERVE,
    FEATURE_TEXT_REPLACE, STATUS_ACTIVATION_FAILED, STATUS_INVALID_HOST, STATUS_OK,
    STATUS_UNAUTHORIZED_FEATURE, STATUS_UNSUPPORTED_FEATURE,
};
use std::cell::Cell;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{OnceLock, RwLock};
use windows::Win32::Graphics::Gdi::{
    CreateFontIndirectW, DeleteObject, GetCurrentObject, GetObjectW, SelectObject, DEFAULT_CHARSET,
    HDC, HGDIOBJ, LOGFONTW, OBJ_FONT,
};

pub const SUPPORTED_FEATURES: u64 =
    FEATURE_TEXT_OBSERVE | FEATURE_TEXT_REPLACE | FEATURE_FONT_SUBSTITUTE;
pub const MAX_TEXT_UNITS: usize = 16 * 1024;
const MAX_FONT_UNITS: usize = 63;

#[derive(Clone, Copy)]
struct HostBridge {
    context: usize,
    decide_utf16: DecideUtf16V1,
}

pub struct DecisionBuffers {
    decision: NativeDecisionV1,
    text: Vec<u16>,
    font: Vec<u16>,
}

static ACTIVE_FEATURES: AtomicU64 = AtomicU64::new(0);
static HOST: OnceLock<RwLock<HostBridge>> = OnceLock::new();

thread_local! {
    static IN_CALLBACK: Cell<bool> = const { Cell::new(false) };
}

pub struct CallbackGuard;

impl CallbackGuard {
    #[must_use]
    pub fn enter() -> Option<Self> {
        IN_CALLBACK.with(|active| (!active.replace(true)).then_some(Self))
    }
}

impl Drop for CallbackGuard {
    fn drop(&mut self) {
        IN_CALLBACK.with(|active| active.set(false));
    }
}

#[must_use]
pub extern "C" fn negotiate_features(requested: u64, granted: u64) -> NativeNegotiationV1 {
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

#[must_use]
// Native Adapter ABI activation callbacks are safe `extern "C"` function pointers. This shared
// boundary validates the host pointer and structure size before copying the table.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn prepare_activation(
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
    if host_ready {
        negotiated
    } else {
        negotiation_error(STATUS_ACTIVATION_FAILED)
    }
}

pub fn commit_activation(active_feature_bits: u64) {
    ACTIVE_FEATURES.store(active_feature_bits, Ordering::Release);
}

#[must_use]
pub extern "C" fn deactivate() -> i32 {
    ACTIVE_FEATURES.store(0, Ordering::Release);
    STATUS_OK
}

#[must_use]
pub fn is_active() -> bool {
    ACTIVE_FEATURES.load(Ordering::Acquire) != 0
}

#[must_use]
pub fn decide(source: &str) -> Option<DecisionBuffers> {
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

impl DecisionBuffers {
    #[must_use]
    pub fn replacement_text(&self) -> Option<&[u16]> {
        let active = ACTIVE_FEATURES.load(Ordering::Acquire);
        (active & FEATURE_TEXT_REPLACE != 0
            && self.decision.decision_bits & DECISION_TEXT_REPLACE != 0)
            .then_some(self.text.as_slice())
    }

    fn replacement_font(&self) -> Option<&[u16]> {
        let active = ACTIVE_FEATURES.load(Ordering::Acquire);
        (active & FEATURE_FONT_SUBSTITUTE != 0
            && self.decision.decision_bits & DECISION_FONT_SUBSTITUTE != 0
            && !self.font.is_empty())
        .then_some(self.font.as_slice())
    }
}

/// Reads an explicit-length or null-terminated UTF-16 argument with a strict upper bound.
///
/// # Safety
///
/// `text` must be readable for the supplied explicit length or through its terminating null.
pub unsafe fn read_text(text: *const u16, length: i32) -> Option<String> {
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
        let length = usize::try_from(length).ok()?;
        (length <= MAX_TEXT_UNITS).then_some(length)?
    };
    (length > 0).then(|| String::from_utf16_lossy(std::slice::from_raw_parts(text, length)))
}

/// Selects a temporary replacement font while calling the original GDI function.
///
/// # Safety
///
/// `hdc` must be valid for the duration of `original`.
pub unsafe fn with_replacement_font<R>(
    hdc: HDC,
    decision: &DecisionBuffers,
    original: impl FnOnce() -> R,
) -> R {
    let mut created_font = None;
    if let Some(family) = decision.replacement_font() {
        let current = GetCurrentObject(hdc, OBJ_FONT);
        let mut logical_font: LOGFONTW = std::mem::zeroed();
        if GetObjectW(
            current,
            std::mem::size_of::<LOGFONTW>() as i32,
            Some(&mut logical_font as *mut _ as *mut core::ffi::c_void),
        ) != 0
        {
            logical_font.lfCharSet = DEFAULT_CHARSET;
            logical_font.lfFaceName.fill(0);
            for (index, unit) in family.iter().copied().take(31).enumerate() {
                logical_font.lfFaceName[index] = unit;
            }
            let font = CreateFontIndirectW(&logical_font);
            if !font.is_invalid() {
                let previous = SelectObject(hdc, HGDIOBJ(font.0));
                if !previous.is_invalid() {
                    created_font = Some((font, previous));
                } else {
                    let _ = DeleteObject(HGDIOBJ(font.0));
                }
            }
        }
    }
    let result = original();
    if let Some((font, previous)) = created_font {
        SelectObject(hdc, previous);
        let _ = DeleteObject(HGDIOBJ(font.0));
    }
    result
}

const fn negotiation_error(status: i32) -> NativeNegotiationV1 {
    NativeNegotiationV1 {
        status,
        active_feature_bits: 0,
    }
}

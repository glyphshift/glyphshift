//! Native `ExtTextOutW` package for the first-party GDI Adapter.

use glyphshift_adapter_gdi::EXT_TEXT_OUT_ADAPTER_ID;
use glyphshift_adapter_gdi_native_support::allows_font_substitution;
use glyphshift_adapter_native_abi::{
    DecideUtf16V1, NativeAdapterApiV1, NativeAdapterDescriptorV1, NativeDecisionV1,
    NativeNegotiationV1, NativeRuntimeHostV1, SourceCharactersUtf16V1, ARCH_X86, ARCH_X86_64,
    DECISION_FONT_SUBSTITUTE, DECISION_TEXT_REPLACE, FEATURE_FONT_SUBSTITUTE, FEATURE_TEXT_OBSERVE,
    FEATURE_TEXT_REPLACE, PLATFORM_WINDOWS, STATUS_ACTIVATION_FAILED, STATUS_INVALID_HOST,
    STATUS_OK, STATUS_UNAUTHORIZED_FEATURE, STATUS_UNSUPPORTED_FEATURE,
};
use retour::GenericDetour;
use std::cell::Cell;
use std::collections::{BTreeSet, HashMap};
use std::ptr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{OnceLock, RwLock};
use windows::core::{s, w, PCWSTR};
use windows::Win32::Foundation::{BOOL, RECT};
use windows::Win32::Graphics::Gdi::{
    CreateFontIndirectW, DeleteObject, GetCurrentObject, GetGlyphIndicesW, GetObjectW,
    SelectObject, DEFAULT_CHARSET, GGI_MARK_NONEXISTING_GLYPHS, HDC, HGDIOBJ, LOGFONTW, OBJ_FONT,
};
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};

const SUPPORTED_FEATURES: u64 =
    FEATURE_TEXT_OBSERVE | FEATURE_TEXT_REPLACE | FEATURE_FONT_SUBSTITUTE;
const ETO_GLYPH_INDEX: u32 = 0x0010;
const MAX_TEXT_UNITS: usize = 16 * 1024;
const MAX_FONT_UNITS: usize = 63;

type FnExtTextOutW =
    unsafe extern "system" fn(HDC, i32, i32, u32, *const RECT, *const u16, u32, *const i32) -> BOOL;

#[derive(Clone, Copy)]
struct HostBridge {
    context: usize,
    decide_utf16: DecideUtf16V1,
    source_characters_utf16: SourceCharactersUtf16V1,
}

static ACTIVE_FEATURES: AtomicU64 = AtomicU64::new(0);
static HOST: OnceLock<RwLock<HostBridge>> = OnceLock::new();
static HOOK: OnceLock<GenericDetour<FnExtTextOutW>> = OnceLock::new();
static GLYPH_CANDIDATES: OnceLock<Vec<(u16, char)>> = OnceLock::new();

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

struct DecisionBuffers {
    decision: NativeDecisionV1,
    text: Vec<u16>,
    font: Vec<u16>,
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
        source_characters_utf16: host.source_characters_utf16,
    };
    let host_ready = if let Some(current) = HOST.get() {
        current.write().map(|mut current| *current = bridge).is_ok()
    } else {
        HOST.set(RwLock::new(bridge)).is_ok()
    };
    if !host_ready {
        return negotiation_error(STATUS_ACTIVATION_FAILED);
    }
    GLYPH_CANDIDATES.get_or_init(load_glyph_candidates);
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

fn load_glyph_candidates() -> Vec<(u16, char)> {
    let mut characters: BTreeSet<char> = (' '..='~').collect();
    if let Some(host) = HOST.get().and_then(|host| host.read().ok()) {
        let mut source_units = vec![0_u16; MAX_TEXT_UNITS];
        let length = (host.source_characters_utf16)(
            host.context as *mut core::ffi::c_void,
            source_units.as_mut_ptr(),
            source_units.len() as u32,
        ) as usize;
        source_units.truncate(length.min(source_units.len()));
        characters.extend(char::decode_utf16(source_units).filter_map(Result::ok));
    }
    characters
        .into_iter()
        .filter_map(|character| {
            let mut units = [0_u16; 2];
            let encoded = character.encode_utf16(&mut units);
            (encoded.len() == 1).then_some((encoded[0], character))
        })
        .collect()
}

unsafe fn decode_glyphs(hdc: HDC, glyphs: &[u16]) -> Option<String> {
    let candidates = GLYPH_CANDIDATES.get()?;
    let characters = candidates.iter().map(|(unit, _)| *unit).collect::<Vec<_>>();
    let mut mapped_glyphs = vec![0_u16; characters.len()];
    if GetGlyphIndicesW(
        hdc,
        PCWSTR(characters.as_ptr()),
        characters.len() as i32,
        mapped_glyphs.as_mut_ptr(),
        GGI_MARK_NONEXISTING_GLYPHS,
    ) == u32::MAX
    {
        return None;
    }
    let mut map = HashMap::<u16, Option<char>>::new();
    for (glyph, (_, character)) in mapped_glyphs.into_iter().zip(candidates) {
        if glyph == 0 || glyph == 0xffff {
            continue;
        }
        map.entry(glyph)
            .and_modify(|mapped| {
                if mapped.is_some_and(|mapped| mapped != *character) {
                    *mapped = None;
                }
            })
            .or_insert(Some(*character));
    }
    glyphs
        .iter()
        .map(|glyph| {
            if *glyph == 3 {
                Some(' ')
            } else {
                map.get(glyph).copied().flatten()
            }
        })
        .collect()
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

#[allow(clippy::too_many_arguments)]
unsafe fn call_with_decision(
    original: &GenericDetour<FnExtTextOutW>,
    hdc: HDC,
    x: i32,
    y: i32,
    options: u32,
    rect: *const RECT,
    original_text: *const u16,
    original_count: u32,
    spacing: *const i32,
    decoded_text: &[u16],
    decision: &DecisionBuffers,
) -> BOOL {
    let active = ACTIVE_FEATURES.load(Ordering::Acquire);
    let replace_text = active & FEATURE_TEXT_REPLACE != 0
        && decision.decision.decision_bits & DECISION_TEXT_REPLACE != 0;
    let replace_font = active & FEATURE_FONT_SUBSTITUTE != 0
        && decision.decision.decision_bits & DECISION_FONT_SUBSTITUTE != 0;
    let mut created_font = None;
    if replace_font && !decision.font.is_empty() {
        let current = GetCurrentObject(hdc, OBJ_FONT);
        let mut logical_font: LOGFONTW = std::mem::zeroed();
        if GetObjectW(
            current,
            std::mem::size_of::<LOGFONTW>() as i32,
            Some(&mut logical_font as *mut _ as *mut core::ffi::c_void),
        ) != 0
            && allows_font_substitution(logical_font.lfCharSet)
        {
            logical_font.lfCharSet = DEFAULT_CHARSET;
            logical_font.lfFaceName.fill(0);
            for (index, unit) in decision.font.iter().copied().take(31).enumerate() {
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
    let font_substituted = created_font.is_some();
    let (text, count, options, spacing) = if replace_text {
        (
            decision.text.as_ptr(),
            decision.text.len() as u32,
            options & !ETO_GLYPH_INDEX,
            ptr::null(),
        )
    } else if font_substituted && options & ETO_GLYPH_INDEX != 0 {
        (
            decoded_text.as_ptr(),
            decoded_text.len() as u32,
            options & !ETO_GLYPH_INDEX,
            ptr::null(),
        )
    } else {
        (original_text, original_count, options, spacing)
    };
    let result = original.call(hdc, x, y, options, rect, text, count, spacing);
    if let Some((font, previous)) = created_font {
        SelectObject(hdc, previous);
        let _ = DeleteObject(HGDIOBJ(font.0));
    }
    result
}

#[allow(clippy::too_many_arguments)]
unsafe extern "system" fn ext_text_out_w_detour(
    hdc: HDC,
    x: i32,
    y: i32,
    options: u32,
    rect: *const RECT,
    text: *const u16,
    count: u32,
    spacing: *const i32,
) -> BOOL {
    let Some(original) = HOOK.get() else {
        return BOOL(0);
    };
    if ACTIVE_FEATURES.load(Ordering::Acquire) == 0 || text.is_null() || count == 0 {
        return original.call(hdc, x, y, options, rect, text, count, spacing);
    }
    let Some(_guard) = CallbackGuard::enter() else {
        return original.call(hdc, x, y, options, rect, text, count, spacing);
    };
    let units = std::slice::from_raw_parts(text, (count as usize).min(MAX_TEXT_UNITS));
    let source = if options & ETO_GLYPH_INDEX != 0 {
        decode_glyphs(hdc, units)
    } else {
        Some(String::from_utf16_lossy(units))
    };
    let Some(source) = source else {
        return original.call(hdc, x, y, options, rect, text, count, spacing);
    };
    let decision = std::panic::catch_unwind(|| decide(source.trim()))
        .ok()
        .flatten();
    let Some(decision) = decision else {
        return original.call(hdc, x, y, options, rect, text, count, spacing);
    };
    let decoded_text = source.encode_utf16().collect::<Vec<_>>();
    call_with_decision(
        original,
        hdc,
        x,
        y,
        options,
        rect,
        text,
        count,
        spacing,
        &decoded_text,
        &decision,
    )
}

unsafe fn install_hook() -> Result<(), ()> {
    let module = GetModuleHandleW(w!("gdi32.dll")).map_err(|_| ())?;
    let address = GetProcAddress(module, s!("ExtTextOutW")).ok_or(())?;
    let target: FnExtTextOutW = std::mem::transmute(address);
    let detour =
        GenericDetour::<FnExtTextOutW>::new(target, ext_text_out_w_detour).map_err(|_| ())?;
    HOOK.set(detour).map_err(|_| ())?;
    HOOK.get().ok_or(())?.enable().map_err(|_| ())
}

#[no_mangle]
pub extern "C" fn glyphshift_adapter_entry_v1() -> NativeAdapterApiV1 {
    NativeAdapterApiV1 {
        struct_size: std::mem::size_of::<NativeAdapterApiV1>() as u32,
        descriptor: NativeAdapterDescriptorV1::inline_target(
            EXT_TEXT_OUT_ADAPTER_ID,
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

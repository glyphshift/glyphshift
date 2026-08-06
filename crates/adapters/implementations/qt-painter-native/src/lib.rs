//! Native Qt 5/6 MSVC x64 `QPainter::drawText` package.

use glyphshift_adapter_native_abi::{
    DecideUtf16V1, NativeAdapterApiV1, NativeAdapterDescriptorV1, NativeDecisionV1,
    NativeNegotiationV1, NativeRuntimeHostV1, ARCH_X86_64, DECISION_TEXT_REPLACE,
    FEATURE_TEXT_OBSERVE, FEATURE_TEXT_REPLACE, PLATFORM_WINDOWS, STATUS_ACTIVATION_FAILED,
    STATUS_INVALID_HOST, STATUS_OK, STATUS_UNAUTHORIZED_FEATURE, STATUS_UNSUPPORTED_FEATURE,
};
use glyphshift_adapter_qt_painter::ADAPTER_ID;
use retour::GenericDetour;
use std::cell::Cell;
use std::ffi::c_void;
use std::mem::MaybeUninit;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{OnceLock, RwLock};
use windows::core::{w, PCSTR};
use windows::Win32::Foundation::HMODULE;
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};

const SUPPORTED_FEATURES: u64 = FEATURE_TEXT_OBSERVE | FEATURE_TEXT_REPLACE;
const MAX_TEXT_UNITS: usize = 16 * 1024;

const DRAW_POINT_SYMBOL: &[u8] = b"?drawText@QPainter@@QEAAXAEBVQPointF@@AEBVQString@@HH@Z\0";
const DRAW_RECT_SYMBOL: &[u8] = b"?drawText@QPainter@@QEAAXAEBVQRect@@HAEBVQString@@PEAV2@@Z\0";
const DRAW_RECT_OPTION_SYMBOL: &[u8] =
    b"?drawText@QPainter@@QEAAXAEBVQRectF@@AEBVQString@@AEBVQTextOption@@@Z\0";
const DRAW_RECT_F_SYMBOL: &[u8] = b"?drawText@QPainter@@QEAAXAEBVQRectF@@HAEBVQString@@PEAV2@@Z\0";
const QSTRING_UTF16_SYMBOL: &[u8] = b"?utf16@QString@@QEBAPEBGXZ\0";
const QSTRING_DTOR_SYMBOL: &[u8] = b"??1QString@@QEAA@XZ\0";
const QSTRING5_CTOR_SYMBOL: &[u8] = b"??0QString@@QEAA@PEBVQChar@@H@Z\0";
const QSTRING5_SIZE_SYMBOL: &[u8] = b"?size@QString@@QEBAHXZ\0";
const QSTRING6_CTOR_SYMBOL: &[u8] = b"??0QString@@QEAA@PEBVQChar@@_J@Z\0";
const QSTRING6_SIZE_SYMBOL: &[u8] = b"?size@QString@@QEBA_JXZ\0";

type RawProc = unsafe extern "system" fn() -> isize;
type FnDrawPoint = unsafe extern "system" fn(*mut c_void, *const c_void, *const c_void, i32, i32);
type FnDrawRect =
    unsafe extern "system" fn(*mut c_void, *const c_void, i32, *const c_void, *mut c_void);
type FnDrawRectOption =
    unsafe extern "system" fn(*mut c_void, *const c_void, *const c_void, *const c_void);
type FnQString5Ctor = unsafe extern "system" fn(*mut c_void, *const u16, i32) -> *mut c_void;
type FnQString6Ctor = unsafe extern "system" fn(*mut c_void, *const u16, i64) -> *mut c_void;
type FnQStringDtor = unsafe extern "system" fn(*mut c_void);
type FnQString5Size = unsafe extern "system" fn(*const c_void) -> i32;
type FnQString6Size = unsafe extern "system" fn(*const c_void) -> i64;
type FnQStringUtf16 = unsafe extern "system" fn(*const c_void) -> *const u16;

#[derive(Clone, Copy)]
struct HostBridge {
    context: usize,
    decide_utf16: DecideUtf16V1,
}

#[derive(Clone, Copy)]
enum QStringApi {
    Qt5 {
        ctor: FnQString5Ctor,
        dtor: FnQStringDtor,
        size: FnQString5Size,
        utf16: FnQStringUtf16,
    },
    Qt6 {
        ctor: FnQString6Ctor,
        dtor: FnQStringDtor,
        size: FnQString6Size,
        utf16: FnQStringUtf16,
    },
}

impl QStringApi {
    unsafe fn read(self, value: *const c_void) -> Option<String> {
        if value.is_null() {
            return None;
        }
        let (length, units) = match self {
            Self::Qt5 { size, utf16, .. } => {
                let length = usize::try_from(size(value)).ok()?;
                (length, utf16(value))
            }
            Self::Qt6 { size, utf16, .. } => {
                let length = usize::try_from(size(value)).ok()?;
                (length, utf16(value))
            }
        };
        if length == 0 || length > MAX_TEXT_UNITS || units.is_null() {
            return None;
        }
        String::from_utf16(std::slice::from_raw_parts(units, length)).ok()
    }

    unsafe fn with_temporary<R>(
        self,
        units: &[u16],
        call: impl FnOnce(*const c_void) -> R,
    ) -> Option<R> {
        if units.is_empty() || units.len() > MAX_TEXT_UNITS {
            return None;
        }
        match self {
            Self::Qt5 { ctor, dtor, .. } => {
                let length = i32::try_from(units.len()).ok()?;
                let mut storage = MaybeUninit::<Qt5StringStorage>::uninit();
                let object = storage.as_mut_ptr().cast::<c_void>();
                ctor(object, units.as_ptr(), length);
                let guard = QStringGuard { object, dtor };
                let result = call(object);
                drop(guard);
                Some(result)
            }
            Self::Qt6 { ctor, dtor, .. } => {
                let length = i64::try_from(units.len()).ok()?;
                let mut storage = MaybeUninit::<Qt6StringStorage>::uninit();
                let object = storage.as_mut_ptr().cast::<c_void>();
                ctor(object, units.as_ptr(), length);
                let guard = QStringGuard { object, dtor };
                let result = call(object);
                drop(guard);
                Some(result)
            }
        }
    }
}

#[repr(C, align(8))]
struct Qt5StringStorage([u8; 64]);

#[repr(C, align(8))]
struct Qt6StringStorage([u8; 64]);

struct QStringGuard {
    object: *mut c_void,
    dtor: FnQStringDtor,
}

impl Drop for QStringGuard {
    fn drop(&mut self) {
        unsafe { (self.dtor)(self.object) };
    }
}

struct QtHooks {
    strings: QStringApi,
    point: GenericDetour<FnDrawPoint>,
    rect: GenericDetour<FnDrawRect>,
    rect_option: GenericDetour<FnDrawRectOption>,
    rect_f: GenericDetour<FnDrawRect>,
}

struct DecisionBuffers {
    decision: NativeDecisionV1,
    text: Vec<u16>,
}

static ACTIVE_FEATURES: AtomicU64 = AtomicU64::new(0);
static HOST: OnceLock<RwLock<HostBridge>> = OnceLock::new();
static HOOKS: OnceLock<QtHooks> = OnceLock::new();

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
    if !host_ready || unsafe { install_hooks() }.is_err() {
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
    let mut font = [];
    let decision = (host.decide_utf16)(
        host.context as *mut c_void,
        source.as_ptr(),
        source.len() as u32,
        text.as_mut_ptr(),
        text.len() as u32,
        font.as_mut_ptr(),
        0,
    );
    if decision.status != STATUS_OK || decision.text_len as usize > text.len() {
        return None;
    }
    text.truncate(decision.text_len as usize);
    Some(DecisionBuffers { decision, text })
}

fn replacement_for(source: &str) -> Option<Vec<u16>> {
    let decision = std::panic::catch_unwind(|| decide(source)).ok().flatten()?;
    let active = ACTIVE_FEATURES.load(Ordering::Acquire);
    (active & FEATURE_TEXT_REPLACE != 0
        && decision.decision.decision_bits & DECISION_TEXT_REPLACE != 0
        && !decision.text.is_empty())
    .then_some(decision.text)
}

fn drawn_point_text(text: &str, from: i32, length: i32) -> Option<String> {
    let from = usize::try_from(from).ok()?;
    let units = text.encode_utf16().collect::<Vec<_>>();
    if from > units.len() || length < -1 {
        return None;
    }
    let end = if length == -1 {
        units.len()
    } else {
        from.checked_add(usize::try_from(length).ok()?)?
            .min(units.len())
    };
    if from == end {
        return None;
    }
    String::from_utf16(&units[from..end]).ok()
}

unsafe extern "system" fn draw_point_detour(
    painter: *mut c_void,
    point: *const c_void,
    text: *const c_void,
    from: i32,
    length: i32,
) {
    let Some(hooks) = HOOKS.get() else {
        return;
    };
    if ACTIVE_FEATURES.load(Ordering::Acquire) == 0 {
        return hooks.point.call(painter, point, text, from, length);
    }
    let Some(_guard) = CallbackGuard::enter() else {
        return hooks.point.call(painter, point, text, from, length);
    };
    let Some(full_text) = hooks.strings.read(text) else {
        return hooks.point.call(painter, point, text, from, length);
    };
    let Some(source) = drawn_point_text(&full_text, from, length) else {
        return hooks.point.call(painter, point, text, from, length);
    };
    let Some(replacement) = replacement_for(&source) else {
        return hooks.point.call(painter, point, text, from, length);
    };
    if hooks
        .strings
        .with_temporary(&replacement, |replacement| {
            hooks.point.call(painter, point, replacement, 0, -1);
        })
        .is_none()
    {
        hooks.point.call(painter, point, text, from, length);
    }
}

unsafe extern "system" fn draw_rect_detour(
    painter: *mut c_void,
    rect: *const c_void,
    flags: i32,
    text: *const c_void,
    bounding_rect: *mut c_void,
) {
    let Some(hooks) = HOOKS.get() else {
        return;
    };
    draw_rect_with(
        hooks,
        text,
        || hooks.rect.call(painter, rect, flags, text, bounding_rect),
        |replacement| {
            hooks
                .rect
                .call(painter, rect, flags, replacement, bounding_rect);
        },
    );
}

unsafe extern "system" fn draw_rect_f_detour(
    painter: *mut c_void,
    rect: *const c_void,
    flags: i32,
    text: *const c_void,
    bounding_rect: *mut c_void,
) {
    let Some(hooks) = HOOKS.get() else {
        return;
    };
    draw_rect_with(
        hooks,
        text,
        || hooks.rect_f.call(painter, rect, flags, text, bounding_rect),
        |replacement| {
            hooks
                .rect_f
                .call(painter, rect, flags, replacement, bounding_rect);
        },
    );
}

unsafe fn draw_rect_with(
    hooks: &QtHooks,
    text: *const c_void,
    original: impl FnOnce(),
    replacement_call: impl FnOnce(*const c_void),
) {
    if ACTIVE_FEATURES.load(Ordering::Acquire) == 0 {
        return original();
    }
    let Some(_guard) = CallbackGuard::enter() else {
        return original();
    };
    let Some(source) = hooks.strings.read(text) else {
        return original();
    };
    let Some(replacement) = replacement_for(&source) else {
        return original();
    };
    if hooks
        .strings
        .with_temporary(&replacement, replacement_call)
        .is_none()
    {
        original();
    }
}

unsafe extern "system" fn draw_rect_option_detour(
    painter: *mut c_void,
    rect: *const c_void,
    text: *const c_void,
    option: *const c_void,
) {
    let Some(hooks) = HOOKS.get() else {
        return;
    };
    draw_rect_with(
        hooks,
        text,
        || hooks.rect_option.call(painter, rect, text, option),
        |replacement| {
            hooks.rect_option.call(painter, rect, replacement, option);
        },
    );
}

unsafe fn resolve(module: HMODULE, symbol: &'static [u8]) -> Result<RawProc, ()> {
    GetProcAddress(module, PCSTR(symbol.as_ptr())).ok_or(())
}

unsafe fn loaded_qt_modules() -> Result<(u8, HMODULE, HMODULE), ()> {
    let qt5 = GetModuleHandleW(w!("Qt5Gui.dll"))
        .ok()
        .zip(GetModuleHandleW(w!("Qt5Core.dll")).ok());
    let qt6 = GetModuleHandleW(w!("Qt6Gui.dll"))
        .ok()
        .zip(GetModuleHandleW(w!("Qt6Core.dll")).ok());
    match (qt5, qt6) {
        (Some((gui, core)), None) => Ok((5, gui, core)),
        (None, Some((gui, core))) => Ok((6, gui, core)),
        _ => Err(()),
    }
}

unsafe fn build_hooks() -> Result<QtHooks, ()> {
    let (major, gui, core) = loaded_qt_modules()?;
    let strings = match major {
        5 => QStringApi::Qt5 {
            ctor: std::mem::transmute::<RawProc, FnQString5Ctor>(resolve(
                core,
                QSTRING5_CTOR_SYMBOL,
            )?),
            dtor: std::mem::transmute::<RawProc, FnQStringDtor>(resolve(
                core,
                QSTRING_DTOR_SYMBOL,
            )?),
            size: std::mem::transmute::<RawProc, FnQString5Size>(resolve(
                core,
                QSTRING5_SIZE_SYMBOL,
            )?),
            utf16: std::mem::transmute::<RawProc, FnQStringUtf16>(resolve(
                core,
                QSTRING_UTF16_SYMBOL,
            )?),
        },
        6 => QStringApi::Qt6 {
            ctor: std::mem::transmute::<RawProc, FnQString6Ctor>(resolve(
                core,
                QSTRING6_CTOR_SYMBOL,
            )?),
            dtor: std::mem::transmute::<RawProc, FnQStringDtor>(resolve(
                core,
                QSTRING_DTOR_SYMBOL,
            )?),
            size: std::mem::transmute::<RawProc, FnQString6Size>(resolve(
                core,
                QSTRING6_SIZE_SYMBOL,
            )?),
            utf16: std::mem::transmute::<RawProc, FnQStringUtf16>(resolve(
                core,
                QSTRING_UTF16_SYMBOL,
            )?),
        },
        _ => return Err(()),
    };
    let point_target =
        std::mem::transmute::<RawProc, FnDrawPoint>(resolve(gui, DRAW_POINT_SYMBOL)?);
    let rect_target = std::mem::transmute::<RawProc, FnDrawRect>(resolve(gui, DRAW_RECT_SYMBOL)?);
    let rect_option_target =
        std::mem::transmute::<RawProc, FnDrawRectOption>(resolve(gui, DRAW_RECT_OPTION_SYMBOL)?);
    let rect_f_target =
        std::mem::transmute::<RawProc, FnDrawRect>(resolve(gui, DRAW_RECT_F_SYMBOL)?);
    Ok(QtHooks {
        strings,
        point: GenericDetour::new(point_target, draw_point_detour).map_err(|_| ())?,
        rect: GenericDetour::new(rect_target, draw_rect_detour).map_err(|_| ())?,
        rect_option: GenericDetour::new(rect_option_target, draw_rect_option_detour)
            .map_err(|_| ())?,
        rect_f: GenericDetour::new(rect_f_target, draw_rect_f_detour).map_err(|_| ())?,
    })
}

unsafe fn install_hooks() -> Result<(), ()> {
    if HOOKS.get().is_none() {
        HOOKS.set(build_hooks()?).map_err(|_| ())?;
    }
    let hooks = HOOKS.get().ok_or(())?;
    if !hooks.point.is_enabled() {
        hooks.point.enable().map_err(|_| ())?;
    }
    if !hooks.rect.is_enabled() {
        hooks.rect.enable().map_err(|_| ())?;
    }
    if !hooks.rect_option.is_enabled() {
        hooks.rect_option.enable().map_err(|_| ())?;
    }
    if !hooks.rect_f.is_enabled() {
        hooks.rect_f.enable().map_err(|_| ())?;
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
            ARCH_X86_64,
        ),
        negotiate_features,
        activate,
        deactivate,
    }
}

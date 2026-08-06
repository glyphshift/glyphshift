//! Native dynamic GTK 3 `gtk_render_layout` package.

use glyphshift_adapter_gtk3_pango::ADAPTER_ID;
use glyphshift_adapter_native_abi::{
    DecideUtf16V1, NativeAdapterApiV1, NativeAdapterDescriptorV1, NativeDecisionV1,
    NativeNegotiationV1, NativeRuntimeHostV1, ARCH_X86_64, DECISION_TEXT_REPLACE,
    FEATURE_TEXT_OBSERVE, FEATURE_TEXT_REPLACE, PLATFORM_WINDOWS, STATUS_ACTIVATION_FAILED,
    STATUS_INVALID_HOST, STATUS_OK, STATUS_UNAUTHORIZED_FEATURE, STATUS_UNSUPPORTED_FEATURE,
};
use retour::GenericDetour;
use std::cell::Cell;
use std::ffi::{c_char, c_void};
use std::mem;
use std::slice;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{OnceLock, RwLock};
use windows::core::PCSTR;
use windows::Win32::Foundation::HMODULE;
use windows::Win32::System::LibraryLoader::GetProcAddress;
use windows::Win32::System::ProcessStatus::K32EnumProcessModules;
use windows::Win32::System::Threading::GetCurrentProcess;

const SUPPORTED_FEATURES: u64 = FEATURE_TEXT_OBSERVE | FEATURE_TEXT_REPLACE;
const MAX_TEXT_BYTES: usize = 64 * 1024;
const MAX_TEXT_UNITS: usize = 64 * 1024;

const GTK_RENDER_LAYOUT_SYMBOL: &[u8] = b"gtk_render_layout\0";
const PANGO_LAYOUT_COPY_SYMBOL: &[u8] = b"pango_layout_copy\0";
const PANGO_LAYOUT_GET_TEXT_SYMBOL: &[u8] = b"pango_layout_get_text\0";
const PANGO_LAYOUT_SET_TEXT_SYMBOL: &[u8] = b"pango_layout_set_text\0";
const PANGO_LAYOUT_GET_ATTRIBUTES_SYMBOL: &[u8] = b"pango_layout_get_attributes\0";
const PANGO_ATTR_LIST_GET_ITERATOR_SYMBOL: &[u8] = b"pango_attr_list_get_iterator\0";
const PANGO_ATTR_ITERATOR_RANGE_SYMBOL: &[u8] = b"pango_attr_iterator_range\0";
const PANGO_ATTR_ITERATOR_DESTROY_SYMBOL: &[u8] = b"pango_attr_iterator_destroy\0";
const G_OBJECT_UNREF_SYMBOL: &[u8] = b"g_object_unref\0";

type RawProc = unsafe extern "system" fn() -> isize;
type FnGtkRenderLayout = unsafe extern "C" fn(*mut c_void, *mut c_void, f64, f64, *mut c_void);
type FnPangoLayoutCopy = unsafe extern "C" fn(*mut c_void) -> *mut c_void;
type FnPangoLayoutGetText = unsafe extern "C" fn(*mut c_void) -> *const c_char;
type FnPangoLayoutSetText = unsafe extern "C" fn(*mut c_void, *const c_char, i32);
type FnPangoLayoutGetAttributes = unsafe extern "C" fn(*mut c_void) -> *mut c_void;
type FnPangoAttrListGetIterator = unsafe extern "C" fn(*mut c_void) -> *mut c_void;
type FnPangoAttrIteratorRange = unsafe extern "C" fn(*mut c_void, *mut i32, *mut i32);
type FnPangoAttrIteratorDestroy = unsafe extern "C" fn(*mut c_void);
type FnGObjectUnref = unsafe extern "C" fn(*mut c_void);

#[derive(Clone, Copy)]
struct HostBridge {
    context: usize,
    decide_utf16: DecideUtf16V1,
}

#[derive(Clone, Copy)]
struct PangoApi {
    copy: FnPangoLayoutCopy,
    get_text: FnPangoLayoutGetText,
    set_text: FnPangoLayoutSetText,
    get_attributes: FnPangoLayoutGetAttributes,
    attr_list_get_iterator: FnPangoAttrListGetIterator,
    attr_iterator_range: FnPangoAttrIteratorRange,
    attr_iterator_destroy: FnPangoAttrIteratorDestroy,
    unref: FnGObjectUnref,
}

impl PangoApi {
    unsafe fn read_text(self, layout: *mut c_void) -> Option<String> {
        if layout.is_null() {
            return None;
        }
        let text = (self.get_text)(layout).cast::<u8>();
        if text.is_null() {
            return None;
        }
        let mut length = 0usize;
        while length <= MAX_TEXT_BYTES && *text.add(length) != 0 {
            length += 1;
        }
        if length == 0 || length > MAX_TEXT_BYTES {
            return None;
        }
        std::str::from_utf8(slice::from_raw_parts(text, length))
            .ok()
            .map(ToOwned::to_owned)
    }

    unsafe fn attributes_cover_full_text(self, layout: *mut c_void) -> bool {
        let attributes = (self.get_attributes)(layout);
        if attributes.is_null() {
            return true;
        }
        let iterator = (self.attr_list_get_iterator)(attributes);
        if iterator.is_null() {
            return false;
        }
        let mut start = 0;
        let mut end = 0;
        (self.attr_iterator_range)(iterator, &mut start, &mut end);
        (self.attr_iterator_destroy)(iterator);
        start == 0 && end == i32::MAX
    }

    unsafe fn copy_with_text(self, layout: *mut c_void, text: &[u8]) -> Option<LayoutGuard> {
        if layout.is_null() || text.is_empty() || text.len() > MAX_TEXT_BYTES || text.contains(&0) {
            return None;
        }
        let length = i32::try_from(text.len()).ok()?;
        let copy = (self.copy)(layout);
        if copy.is_null() {
            return None;
        }
        let mut terminated = Vec::with_capacity(text.len() + 1);
        terminated.extend_from_slice(text);
        terminated.push(0);
        (self.set_text)(copy, terminated.as_ptr().cast(), length);
        Some(LayoutGuard {
            layout: copy,
            unref: self.unref,
        })
    }
}

struct LayoutGuard {
    layout: *mut c_void,
    unref: FnGObjectUnref,
}

impl Drop for LayoutGuard {
    fn drop(&mut self) {
        unsafe { (self.unref)(self.layout) };
    }
}

struct Gtk3Hooks {
    pango: PangoApi,
    render_layout: GenericDetour<FnGtkRenderLayout>,
}

struct DecisionBuffers {
    decision: NativeDecisionV1,
    text: Vec<u16>,
}

static ACTIVE_FEATURES: AtomicU64 = AtomicU64::new(0);
static HOST: OnceLock<RwLock<HostBridge>> = OnceLock::new();
static HOOKS: OnceLock<Gtk3Hooks> = OnceLock::new();

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
        || unsafe { (*host).struct_size } != mem::size_of::<NativeRuntimeHostV1>() as u32
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
    if source.len() > MAX_TEXT_UNITS {
        return None;
    }
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

fn replacement_for(source: &str) -> Option<Vec<u8>> {
    let decision = std::panic::catch_unwind(|| decide(source)).ok().flatten()?;
    let active = ACTIVE_FEATURES.load(Ordering::Acquire);
    if active & FEATURE_TEXT_REPLACE == 0
        || decision.decision.decision_bits & DECISION_TEXT_REPLACE == 0
        || decision.text.is_empty()
    {
        return None;
    }
    let replacement = String::from_utf16(&decision.text).ok()?.into_bytes();
    (!replacement.contains(&0) && replacement.len() <= MAX_TEXT_BYTES).then_some(replacement)
}

unsafe extern "C" fn render_layout_detour(
    context: *mut c_void,
    cairo: *mut c_void,
    x: f64,
    y: f64,
    layout: *mut c_void,
) {
    let Some(hooks) = HOOKS.get() else {
        return;
    };
    if ACTIVE_FEATURES.load(Ordering::Acquire) == 0 {
        return hooks.render_layout.call(context, cairo, x, y, layout);
    }
    let Some(_guard) = CallbackGuard::enter() else {
        return hooks.render_layout.call(context, cairo, x, y, layout);
    };
    if !hooks.pango.attributes_cover_full_text(layout) {
        return hooks.render_layout.call(context, cairo, x, y, layout);
    }
    let Some(source) = hooks.pango.read_text(layout) else {
        return hooks.render_layout.call(context, cairo, x, y, layout);
    };
    let Some(replacement) = replacement_for(&source) else {
        return hooks.render_layout.call(context, cairo, x, y, layout);
    };
    let Some(replacement_layout) = hooks.pango.copy_with_text(layout, &replacement) else {
        return hooks.render_layout.call(context, cairo, x, y, layout);
    };
    hooks
        .render_layout
        .call(context, cairo, x, y, replacement_layout.layout);
}

unsafe fn loaded_modules() -> Result<Vec<HMODULE>, ()> {
    let process = GetCurrentProcess();
    let mut modules = vec![HMODULE::default(); 128];
    loop {
        let byte_capacity = modules
            .len()
            .checked_mul(mem::size_of::<HMODULE>())
            .and_then(|size| u32::try_from(size).ok())
            .ok_or(())?;
        let mut needed = 0u32;
        if !K32EnumProcessModules(process, modules.as_mut_ptr(), byte_capacity, &mut needed)
            .as_bool()
        {
            return Err(());
        }
        let count = usize::try_from(needed)
            .ok()
            .and_then(|bytes| bytes.checked_div(mem::size_of::<HMODULE>()))
            .ok_or(())?;
        if count <= modules.len() {
            modules.truncate(count);
            return Ok(modules);
        }
        modules.resize(count, HMODULE::default());
    }
}

unsafe fn module_with_exports(symbols: &[&'static [u8]]) -> Result<HMODULE, ()> {
    let mut matched = None;
    for module in loaded_modules()? {
        let has_all = symbols
            .iter()
            .all(|symbol| GetProcAddress(module, PCSTR(symbol.as_ptr())).is_some());
        if has_all {
            if matched.is_some() {
                return Err(());
            }
            matched = Some(module);
        }
    }
    matched.ok_or(())
}

unsafe fn resolve(module: HMODULE, symbol: &'static [u8]) -> Result<RawProc, ()> {
    GetProcAddress(module, PCSTR(symbol.as_ptr())).ok_or(())
}

unsafe fn build_hooks() -> Result<Gtk3Hooks, ()> {
    let gtk = module_with_exports(&[GTK_RENDER_LAYOUT_SYMBOL])?;
    let pango = module_with_exports(&[
        PANGO_LAYOUT_COPY_SYMBOL,
        PANGO_LAYOUT_GET_TEXT_SYMBOL,
        PANGO_LAYOUT_SET_TEXT_SYMBOL,
        PANGO_LAYOUT_GET_ATTRIBUTES_SYMBOL,
        PANGO_ATTR_LIST_GET_ITERATOR_SYMBOL,
        PANGO_ATTR_ITERATOR_RANGE_SYMBOL,
        PANGO_ATTR_ITERATOR_DESTROY_SYMBOL,
    ])?;
    let gobject = module_with_exports(&[G_OBJECT_UNREF_SYMBOL])?;

    let pango = PangoApi {
        copy: mem::transmute::<RawProc, FnPangoLayoutCopy>(resolve(
            pango,
            PANGO_LAYOUT_COPY_SYMBOL,
        )?),
        get_text: mem::transmute::<RawProc, FnPangoLayoutGetText>(resolve(
            pango,
            PANGO_LAYOUT_GET_TEXT_SYMBOL,
        )?),
        set_text: mem::transmute::<RawProc, FnPangoLayoutSetText>(resolve(
            pango,
            PANGO_LAYOUT_SET_TEXT_SYMBOL,
        )?),
        get_attributes: mem::transmute::<RawProc, FnPangoLayoutGetAttributes>(resolve(
            pango,
            PANGO_LAYOUT_GET_ATTRIBUTES_SYMBOL,
        )?),
        attr_list_get_iterator: mem::transmute::<RawProc, FnPangoAttrListGetIterator>(resolve(
            pango,
            PANGO_ATTR_LIST_GET_ITERATOR_SYMBOL,
        )?),
        attr_iterator_range: mem::transmute::<RawProc, FnPangoAttrIteratorRange>(resolve(
            pango,
            PANGO_ATTR_ITERATOR_RANGE_SYMBOL,
        )?),
        attr_iterator_destroy: mem::transmute::<RawProc, FnPangoAttrIteratorDestroy>(resolve(
            pango,
            PANGO_ATTR_ITERATOR_DESTROY_SYMBOL,
        )?),
        unref: mem::transmute::<RawProc, FnGObjectUnref>(resolve(gobject, G_OBJECT_UNREF_SYMBOL)?),
    };
    let render_layout =
        mem::transmute::<RawProc, FnGtkRenderLayout>(resolve(gtk, GTK_RENDER_LAYOUT_SYMBOL)?);
    Ok(Gtk3Hooks {
        pango,
        render_layout: GenericDetour::new(render_layout, render_layout_detour).map_err(|_| ())?,
    })
}

unsafe fn install_hooks() -> Result<(), ()> {
    if HOOKS.get().is_none() {
        HOOKS.set(build_hooks()?).map_err(|_| ())?;
    }
    let hooks = HOOKS.get().ok_or(())?;
    if !hooks.render_layout.is_enabled() {
        hooks.render_layout.enable().map_err(|_| ())?;
    }
    Ok(())
}

#[no_mangle]
pub extern "C" fn glyphshift_adapter_entry_v1() -> NativeAdapterApiV1 {
    NativeAdapterApiV1 {
        struct_size: mem::size_of::<NativeAdapterApiV1>() as u32,
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

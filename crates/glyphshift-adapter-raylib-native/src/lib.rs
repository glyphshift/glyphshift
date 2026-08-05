//! Native dynamic raylib 5.5-compatible `DrawTextEx` package.

use glyphshift_adapter_native_abi::{
    DecideUtf16V1, NativeAdapterApiV1, NativeAdapterDescriptorV1, NativeDecisionV1,
    NativeNegotiationV1, NativeRuntimeHostV1, ARCH_X86_64, DECISION_TEXT_REPLACE,
    FEATURE_TEXT_OBSERVE, FEATURE_TEXT_REPLACE, PLATFORM_WINDOWS, STATUS_ACTIVATION_FAILED,
    STATUS_INVALID_HOST, STATUS_OK, STATUS_UNAUTHORIZED_FEATURE, STATUS_UNSUPPORTED_FEATURE,
};
use glyphshift_adapter_raylib::{ADAPTER_ID, MAX_FALLBACK_GLYPHS, MAX_TEXT_BYTES};
use retour::GenericDetour;
use std::cell::Cell;
use std::collections::BTreeSet;
use std::ffi::{c_char, c_void, CStr, CString};
use std::mem;
use std::path::{Path, PathBuf};
use std::ptr::null_mut;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock, RwLock};
use windows::core::{w, PCSTR};
use windows::Win32::Foundation::HMODULE;
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
use windows::Win32::System::Threading::GetCurrentThreadId;

const SUPPORTED_FEATURES: u64 = FEATURE_TEXT_OBSERVE | FEATURE_TEXT_REPLACE;
const MAX_TEXT_UNITS: usize = 64 * 1024;
const FALLBACK_RASTER_SIZE: i32 = 48;
const FONT_PADDING: i32 = 4;
const FONT_DEFAULT: i32 = 0;
const TEXTURE_FILTER_BILINEAR: i32 = 1;

const DRAW_TEXT_EX_SYMBOL: &[u8] = b"DrawTextEx\0";
const END_DRAWING_SYMBOL: &[u8] = b"EndDrawing\0";
const CLOSE_WINDOW_SYMBOL: &[u8] = b"CloseWindow\0";
const LOAD_FILE_DATA_SYMBOL: &[u8] = b"LoadFileData\0";
const UNLOAD_FILE_DATA_SYMBOL: &[u8] = b"UnloadFileData\0";
const LOAD_FONT_DATA_SYMBOL: &[u8] = b"LoadFontData\0";
const UNLOAD_FONT_DATA_SYMBOL: &[u8] = b"UnloadFontData\0";
const GEN_IMAGE_FONT_ATLAS_SYMBOL: &[u8] = b"GenImageFontAtlas\0";
const LOAD_TEXTURE_FROM_IMAGE_SYMBOL: &[u8] = b"LoadTextureFromImage\0";
const UNLOAD_IMAGE_SYMBOL: &[u8] = b"UnloadImage\0";
const GEN_TEXTURE_MIPMAPS_SYMBOL: &[u8] = b"GenTextureMipmaps\0";
const SET_TEXTURE_FILTER_SYMBOL: &[u8] = b"SetTextureFilter\0";
const UNLOAD_FONT_SYMBOL: &[u8] = b"UnloadFont\0";
const MEM_FREE_SYMBOL: &[u8] = b"MemFree\0";

#[repr(C)]
#[derive(Clone, Copy)]
struct Vector2 {
    x: f32,
    y: f32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Color {
    r: u8,
    g: u8,
    b: u8,
    a: u8,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Rectangle {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Image {
    data: *mut c_void,
    width: i32,
    height: i32,
    mipmaps: i32,
    format: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Texture {
    id: u32,
    width: i32,
    height: i32,
    mipmaps: i32,
    format: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct GlyphInfo {
    value: i32,
    offset_x: i32,
    offset_y: i32,
    advance_x: i32,
    image: Image,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Font {
    base_size: i32,
    glyph_count: i32,
    glyph_padding: i32,
    texture: Texture,
    recs: *mut Rectangle,
    glyphs: *mut GlyphInfo,
}

unsafe impl Send for Font {}

type FnDrawTextEx = unsafe extern "C" fn(Font, *const c_char, Vector2, f32, f32, Color);
type FnEndDrawing = unsafe extern "C" fn();
type FnCloseWindow = unsafe extern "C" fn();
type FnLoadFileData = unsafe extern "C" fn(*const c_char, *mut i32) -> *mut u8;
type FnUnloadFileData = unsafe extern "C" fn(*mut u8);
type FnLoadFontData =
    unsafe extern "C" fn(*const u8, i32, i32, *mut i32, i32, i32) -> *mut GlyphInfo;
type FnUnloadFontData = unsafe extern "C" fn(*mut GlyphInfo, i32);
type FnGenImageFontAtlas =
    unsafe extern "C" fn(*const GlyphInfo, *mut *mut Rectangle, i32, i32, i32, i32) -> Image;
type FnLoadTextureFromImage = unsafe extern "C" fn(Image) -> Texture;
type FnUnloadImage = unsafe extern "C" fn(Image);
type FnGenTextureMipmaps = unsafe extern "C" fn(*mut Texture);
type FnSetTextureFilter = unsafe extern "C" fn(Texture, i32);
type FnUnloadFont = unsafe extern "C" fn(Font);
type FnMemFree = unsafe extern "C" fn(*mut c_void);

#[derive(Clone, Copy)]
struct HostBridge {
    context: usize,
    decide_utf16: DecideUtf16V1,
}

#[derive(Clone, Copy)]
struct RaylibApi {
    load_file_data: FnLoadFileData,
    unload_file_data: FnUnloadFileData,
    load_font_data: FnLoadFontData,
    unload_font_data: FnUnloadFontData,
    gen_image_font_atlas: FnGenImageFontAtlas,
    load_texture_from_image: FnLoadTextureFromImage,
    unload_image: FnUnloadImage,
    gen_texture_mipmaps: FnGenTextureMipmaps,
    set_texture_filter: FnSetTextureFilter,
    unload_font: FnUnloadFont,
    mem_free: FnMemFree,
}

struct RaylibHooks {
    api: RaylibApi,
    draw_text_ex: GenericDetour<FnDrawTextEx>,
    end_drawing: GenericDetour<FnEndDrawing>,
    close_window: GenericDetour<FnCloseWindow>,
}

struct AtlasCache {
    font: Font,
    codepoints: BTreeSet<i32>,
    font_path: PathBuf,
}

struct RuntimeState {
    render_thread: Option<u32>,
    active: Option<AtlasCache>,
    retired: Vec<Font>,
}

impl RuntimeState {
    const fn new() -> Self {
        Self {
            render_thread: None,
            active: None,
            retired: Vec::new(),
        }
    }

    fn bind_render_thread(&mut self) -> bool {
        let thread = unsafe { GetCurrentThreadId() };
        match self.render_thread {
            Some(bound) => bound == thread,
            None => {
                self.render_thread = Some(thread);
                true
            }
        }
    }

    fn retire_active(&mut self) {
        if let Some(active) = self.active.take() {
            self.retired.push(active.font);
        }
    }
}

struct DecisionBuffers {
    decision: NativeDecisionV1,
    text: Vec<u16>,
}

static ACTIVE_FEATURES: AtomicU64 = AtomicU64::new(0);
static HOST: OnceLock<RwLock<HostBridge>> = OnceLock::new();
static FALLBACK_FONT: OnceLock<RwLock<PathBuf>> = OnceLock::new();
static HOOKS: OnceLock<RaylibHooks> = OnceLock::new();
static STATE: Mutex<RuntimeState> = Mutex::new(RuntimeState::new());

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

const fn negotiation_error(status: i32) -> NativeNegotiationV1 {
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
    if !host_ready {
        return negotiation_error(STATUS_ACTIVATION_FAILED);
    }

    if negotiated.active_feature_bits & FEATURE_TEXT_REPLACE != 0 {
        let Some(path) = resolve_fallback_font() else {
            return negotiation_error(STATUS_ACTIVATION_FAILED);
        };
        let font_ready = if let Some(current) = FALLBACK_FONT.get() {
            current.write().map(|mut current| *current = path).is_ok()
        } else {
            FALLBACK_FONT.set(RwLock::new(path)).is_ok()
        };
        if !font_ready {
            return negotiation_error(STATUS_ACTIVATION_FAILED);
        }
    }

    if unsafe { install_hooks() }.is_err() {
        return negotiation_error(STATUS_ACTIVATION_FAILED);
    }
    ACTIVE_FEATURES.store(negotiated.active_feature_bits, Ordering::Release);
    negotiated
}

extern "C" fn deactivate() -> i32 {
    ACTIVE_FEATURES.store(0, Ordering::Release);
    STATUS_OK
}

fn resolve_fallback_font() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("GLYPHSHIFT_RAYLIB_FALLBACK_FONT") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Some(path);
        }
    }
    let windows = PathBuf::from(std::env::var_os("WINDIR")?);
    ["Deng.ttf", "Dengl.ttf", "Dengb.ttf"]
        .into_iter()
        .map(|name| windows.join("Fonts").join(name))
        .find(|path| path.is_file())
}

fn decide(source: &str) -> Option<DecisionBuffers> {
    let host = *HOST.get()?.read().ok()?;
    let source = source.encode_utf16().collect::<Vec<_>>();
    if source.is_empty() || source.len() > MAX_TEXT_UNITS {
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
    let replacement = normalize_raylib_text(&String::from_utf16(&decision.text).ok()?).into_bytes();
    (!replacement.contains(&0) && replacement.len() <= MAX_TEXT_BYTES).then_some(replacement)
}

fn normalize_raylib_text(text: &str) -> String {
    text.replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\t', " ")
}

unsafe fn read_source(text: *const c_char) -> Option<String> {
    if text.is_null() {
        return None;
    }
    let bytes = CStr::from_ptr(text).to_bytes();
    if bytes.is_empty() || bytes.len() > MAX_TEXT_BYTES {
        return None;
    }
    std::str::from_utf8(bytes).ok().map(ToOwned::to_owned)
}

fn font_abi_plausible(font: Font) -> bool {
    (1..=4096).contains(&font.base_size)
        && font.glyph_count > 0
        && font.texture.id != 0
        && !font.recs.is_null()
        && !font.glyphs.is_null()
}

unsafe fn missing_loaded_glyphs(
    glyphs: *mut GlyphInfo,
    count: usize,
    required: &BTreeSet<i32>,
) -> Vec<i32> {
    if glyphs.is_null() || count != required.len() {
        return required.iter().copied().collect();
    }
    let glyphs = std::slice::from_raw_parts(glyphs, count);
    required
        .iter()
        .filter(|codepoint| {
            !glyphs.iter().any(|glyph| {
                glyph.value == **codepoint
                    && !glyph.image.data.is_null()
                    && glyph.image.width > 0
                    && glyph.image.height > 0
            })
        })
        .copied()
        .collect()
}

unsafe fn atlas_rects_valid(image: Image, recs: *mut Rectangle, count: usize) -> bool {
    if image.data.is_null() || image.width <= 0 || image.height <= 0 || recs.is_null() {
        return false;
    }
    let recs = std::slice::from_raw_parts(recs, count);
    for (index, rec) in recs.iter().enumerate() {
        if rec.width <= 0.0
            || rec.height <= 0.0
            || rec.x < 0.0
            || rec.y < 0.0
            || rec.x + rec.width > image.width as f32
            || rec.y + rec.height > image.height as f32
        {
            return false;
        }
        for other in &recs[..index] {
            let overlaps = rec.x < other.x + other.width
                && rec.x + rec.width > other.x
                && rec.y < other.y + other.height
                && rec.y + rec.height > other.y;
            if overlaps {
                return false;
            }
        }
    }
    true
}

unsafe fn build_fallback_font(
    api: RaylibApi,
    font_path: &Path,
    codepoints: &BTreeSet<i32>,
) -> Option<Font> {
    if codepoints.is_empty() || codepoints.len() > MAX_FALLBACK_GLYPHS {
        return None;
    }
    let font_path = CString::new(font_path.to_string_lossy().as_bytes()).ok()?;
    let mut data_size = 0;
    let file_data = (api.load_file_data)(font_path.as_ptr(), &mut data_size);
    if file_data.is_null() || data_size <= 0 {
        return None;
    }
    let mut requested = codepoints.iter().copied().collect::<Vec<_>>();
    let glyphs = (api.load_font_data)(
        file_data,
        data_size,
        FALLBACK_RASTER_SIZE,
        requested.as_mut_ptr(),
        requested.len() as i32,
        FONT_DEFAULT,
    );
    if !missing_loaded_glyphs(glyphs, requested.len(), codepoints).is_empty() {
        if !glyphs.is_null() {
            (api.unload_font_data)(glyphs, requested.len() as i32);
        }
        (api.unload_file_data)(file_data);
        return None;
    }

    let mut recs = null_mut();
    let atlas = (api.gen_image_font_atlas)(
        glyphs,
        &mut recs,
        requested.len() as i32,
        FALLBACK_RASTER_SIZE,
        FONT_PADDING,
        1,
    );
    if !atlas_rects_valid(atlas, recs, requested.len()) {
        if !atlas.data.is_null() {
            (api.unload_image)(atlas);
        }
        if !recs.is_null() {
            (api.mem_free)(recs.cast());
        }
        (api.unload_font_data)(glyphs, requested.len() as i32);
        (api.unload_file_data)(file_data);
        return None;
    }

    let mut texture = (api.load_texture_from_image)(atlas);
    (api.unload_image)(atlas);
    (api.unload_file_data)(file_data);
    if texture.id == 0 {
        (api.mem_free)(recs.cast());
        (api.unload_font_data)(glyphs, requested.len() as i32);
        return None;
    }
    (api.gen_texture_mipmaps)(&mut texture);
    (api.set_texture_filter)(texture, TEXTURE_FILTER_BILINEAR);
    Some(Font {
        base_size: FALLBACK_RASTER_SIZE,
        glyph_count: requested.len() as i32,
        glyph_padding: FONT_PADDING,
        texture,
        recs,
        glyphs,
    })
}

unsafe extern "C" fn draw_text_ex_detour(
    font: Font,
    text: *const c_char,
    position: Vector2,
    font_size: f32,
    spacing: f32,
    tint: Color,
) {
    let Some(hooks) = HOOKS.get() else {
        return;
    };
    let original = &hooks.draw_text_ex;
    if ACTIVE_FEATURES.load(Ordering::Acquire) == 0 {
        if let Ok(mut state) = STATE.lock() {
            if state.bind_render_thread() {
                state.retire_active();
            }
        }
        return original.call(font, text, position, font_size, spacing, tint);
    }
    let Some(_guard) = CallbackGuard::enter() else {
        return original.call(font, text, position, font_size, spacing, tint);
    };
    let Some(source) = read_source(text) else {
        return original.call(font, text, position, font_size, spacing, tint);
    };
    let Some(replacement) = replacement_for(&source) else {
        return original.call(font, text, position, font_size, spacing, tint);
    };
    if !font_abi_plausible(font) {
        return original.call(font, text, position, font_size, spacing, tint);
    }
    let Ok(replacement_text) = std::str::from_utf8(&replacement) else {
        return original.call(font, text, position, font_size, spacing, tint);
    };
    let required = replacement_text
        .chars()
        .filter(|character| *character != '\n')
        .map(|character| character as i32)
        .collect::<BTreeSet<_>>();
    if required.is_empty() {
        let Ok(replacement) = CString::new(replacement) else {
            return original.call(font, text, position, font_size, spacing, tint);
        };
        return original.call(
            font,
            replacement.as_ptr(),
            position,
            font_size,
            spacing,
            tint,
        );
    }
    let Some(font_path) = FALLBACK_FONT
        .get()
        .and_then(|path| path.read().ok())
        .map(|path| path.clone())
    else {
        return original.call(font, text, position, font_size, spacing, tint);
    };

    let Ok(mut state) = STATE.lock() else {
        return original.call(font, text, position, font_size, spacing, tint);
    };
    if !state.bind_render_thread() {
        drop(state);
        return original.call(font, text, position, font_size, spacing, tint);
    }
    let cache_ready = state.active.as_ref().is_some_and(|active| {
        active.font_path == font_path && required.is_subset(&active.codepoints)
    });
    if !cache_ready {
        let mut desired = required.clone();
        if let Some(active) = &state.active {
            if active.font_path == font_path {
                desired.extend(active.codepoints.iter().copied());
            }
        }
        if desired.len() > MAX_FALLBACK_GLYPHS {
            drop(state);
            return original.call(font, text, position, font_size, spacing, tint);
        }
        let Some(new_font) = build_fallback_font(hooks.api, &font_path, &desired) else {
            drop(state);
            return original.call(font, text, position, font_size, spacing, tint);
        };
        state.retire_active();
        state.active = Some(AtlasCache {
            font: new_font,
            codepoints: desired,
            font_path: font_path.clone(),
        });
    }
    if ACTIVE_FEATURES.load(Ordering::Acquire) & FEATURE_TEXT_REPLACE == 0 {
        state.retire_active();
        drop(state);
        return original.call(font, text, position, font_size, spacing, tint);
    }
    let Some(replacement_font) = state.active.as_ref().map(|active| active.font) else {
        drop(state);
        return original.call(font, text, position, font_size, spacing, tint);
    };
    drop(state);
    let Ok(replacement) = CString::new(replacement) else {
        return original.call(font, text, position, font_size, spacing, tint);
    };
    original.call(
        replacement_font,
        replacement.as_ptr(),
        position,
        font_size,
        spacing,
        tint,
    );
}

unsafe extern "C" fn end_drawing_detour() {
    let Some(hooks) = HOOKS.get() else {
        return;
    };
    hooks.end_drawing.call();
    let Ok(mut state) = STATE.lock() else {
        return;
    };
    if !state.bind_render_thread() {
        return;
    }
    if ACTIVE_FEATURES.load(Ordering::Acquire) == 0 {
        state.retire_active();
    }
    let retired = mem::take(&mut state.retired);
    drop(state);
    for font in retired {
        (hooks.api.unload_font)(font);
    }
}

unsafe extern "C" fn close_window_detour() {
    let Some(hooks) = HOOKS.get() else {
        return;
    };
    if let Ok(mut state) = STATE.lock() {
        if state.bind_render_thread() {
            let active = state.active.take().map(|active| active.font);
            let retired = mem::take(&mut state.retired);
            state.render_thread = None;
            drop(state);
            if let Some(font) = active {
                (hooks.api.unload_font)(font);
            }
            for font in retired {
                (hooks.api.unload_font)(font);
            }
        }
    }
    hooks.close_window.call();
}

unsafe fn resolve<T: Copy>(module: HMODULE, symbol: &'static [u8]) -> Option<T> {
    let address = GetProcAddress(module, PCSTR(symbol.as_ptr()));
    address.map(|address| mem::transmute_copy::<_, T>(&address))
}

unsafe fn install_hooks() -> Result<(), ()> {
    if HOOKS.get().is_some() {
        return Ok(());
    }
    let module = GetModuleHandleW(w!("raylib.dll")).map_err(|_| ())?;
    let api = RaylibApi {
        load_file_data: resolve(module, LOAD_FILE_DATA_SYMBOL).ok_or(())?,
        unload_file_data: resolve(module, UNLOAD_FILE_DATA_SYMBOL).ok_or(())?,
        load_font_data: resolve(module, LOAD_FONT_DATA_SYMBOL).ok_or(())?,
        unload_font_data: resolve(module, UNLOAD_FONT_DATA_SYMBOL).ok_or(())?,
        gen_image_font_atlas: resolve(module, GEN_IMAGE_FONT_ATLAS_SYMBOL).ok_or(())?,
        load_texture_from_image: resolve(module, LOAD_TEXTURE_FROM_IMAGE_SYMBOL).ok_or(())?,
        unload_image: resolve(module, UNLOAD_IMAGE_SYMBOL).ok_or(())?,
        gen_texture_mipmaps: resolve(module, GEN_TEXTURE_MIPMAPS_SYMBOL).ok_or(())?,
        set_texture_filter: resolve(module, SET_TEXTURE_FILTER_SYMBOL).ok_or(())?,
        unload_font: resolve(module, UNLOAD_FONT_SYMBOL).ok_or(())?,
        mem_free: resolve(module, MEM_FREE_SYMBOL).ok_or(())?,
    };
    let draw: FnDrawTextEx = resolve(module, DRAW_TEXT_EX_SYMBOL).ok_or(())?;
    let end: FnEndDrawing = resolve(module, END_DRAWING_SYMBOL).ok_or(())?;
    let close: FnCloseWindow = resolve(module, CLOSE_WINDOW_SYMBOL).ok_or(())?;
    let hooks = RaylibHooks {
        api,
        draw_text_ex: GenericDetour::new(draw, draw_text_ex_detour).map_err(|_| ())?,
        end_drawing: GenericDetour::new(end, end_drawing_detour).map_err(|_| ())?,
        close_window: GenericDetour::new(close, close_window_detour).map_err(|_| ())?,
    };
    HOOKS.set(hooks).map_err(|_| ())?;
    let hooks = HOOKS.get().ok_or(())?;
    hooks.draw_text_ex.enable().map_err(|_| ())?;
    hooks.end_drawing.enable().map_err(|_| ())?;
    hooks.close_window.enable().map_err(|_| ())?;
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

#[cfg(test)]
mod tests {
    use super::normalize_raylib_text;

    #[test]
    fn normalizes_controls_to_raylib_draw_text_ex_semantics() {
        assert_eq!(
            normalize_raylib_text("first\r\nsecond\rthird\tfourth"),
            "first\nsecond\nthird fourth"
        );
    }
}

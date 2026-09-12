use glyphshift_adapter_gdi::{GdiCall, GdiFont, GdiGlyphMap, GdiInlineAdapter, PreparedGdiCall};
use glyphshift_adapter_gdiplus::{
    GdiPlusCall, GdiPlusFont, GdiPlusInlineAdapter, PreparedGdiPlusCall,
};
use glyphshift_domain::{FontDecision, Generation, RenderDecision, TextDecision};
use std::ffi::c_void;
use std::mem::size_of;
use std::ptr::{null, null_mut};
use windows::core::{w, Interface};
use windows::Win32::Foundation::RECT as WindowsRect;
use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_ALPHA_MODE_IGNORE, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_PIXEL_FORMAT,
    D2D_POINT_2F, D2D_RECT_F,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1CreateFactory, ID2D1Factory, ID2D1RenderTarget,
    D2D1_BITMAP_INTERPOLATION_MODE_NEAREST_NEIGHBOR, D2D1_COMPATIBLE_RENDER_TARGET_OPTIONS_NONE,
    D2D1_DRAW_TEXT_OPTIONS_NONE, D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_FEATURE_LEVEL_DEFAULT,
    D2D1_RENDER_TARGET_PROPERTIES, D2D1_RENDER_TARGET_TYPE_DEFAULT, D2D1_RENDER_TARGET_USAGE_NONE,
};
use windows::Win32::Graphics::DirectWrite::{
    DWriteCreateFactory, IDWriteFactory, IDWriteTextLayout1, DWRITE_FACTORY_TYPE_SHARED,
    DWRITE_FONT_FEATURE, DWRITE_FONT_FEATURE_TAG_STANDARD_LIGATURES, DWRITE_FONT_STRETCH_NORMAL,
    DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT_BOLD, DWRITE_FONT_WEIGHT_NORMAL,
    DWRITE_MEASURING_MODE_NATURAL, DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
    DWRITE_TEXT_ALIGNMENT_TRAILING, DWRITE_TEXT_RANGE,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Gdi::HDC as WindowsHdc;
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, GUID_WICPixelFormat32bppPBGRA, IWICImagingFactory,
    WICBitmapCacheOnLoad,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
};
use windows_sys::Win32::Foundation::RECT;
use windows_sys::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, CreateFontIndirectW, DeleteDC, DeleteObject, DrawTextW,
    ExtTextOutW, GetGlyphIndicesW, SelectObject, SetBkMode, SetTextColor, TextOutW, BITMAPINFO,
    BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, DT_LEFT, DT_SINGLELINE, GGI_MARK_NONEXISTING_GLYPHS,
    HBITMAP, HDC, HGDIOBJ, LOGFONTW, SYMBOL_CHARSET, TRANSPARENT,
};
use windows_sys::Win32::Graphics::GdiPlus::{
    GdipCreateFont, GdipCreateFontFamilyFromName, GdipCreateFromHDC, GdipCreateSolidFill,
    GdipDeleteBrush, GdipDeleteFont, GdipDeleteFontFamily, GdipDeleteGraphics, GdipDrawString,
    GdipGraphicsClear, GdiplusShutdown, GdiplusStartup, GdiplusStartupInput, GpBrush, GpFont,
    GpFontFamily, GpGraphics, GpSolidFill, RectF,
};
use windows_sys::Win32::System::Console::{GetStdHandle, WriteConsoleW, STD_OUTPUT_HANDLE};

const WIDTH: i32 = 360;
const HEIGHT: i32 = 96;
const PIXEL_COUNT: usize = WIDTH as usize * HEIGHT as usize;
const WHITE: u32 = 0xffff_ffff;
const BLACK: u32 = 0xff00_0000;

/// Attempts one Unicode Console write without falling back to a byte-oriented API.
///
/// The synthetic process-family contract intentionally permits a redirected output handle:
/// the return value records whether Windows accepted the write, while a target-process
/// observer can still prove which process entered `WriteConsoleW`.
#[must_use]
pub fn write_raw_console(text: &str) -> bool {
    let units = text.encode_utf16().collect::<Vec<_>>();
    let mut written = 0_u32;
    unsafe {
        WriteConsoleW(
            GetStdHandle(STD_OUTPUT_HANDLE),
            units.as_ptr().cast(),
            units.len() as u32,
            &mut written,
            null(),
        ) != 0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PixelEvidence {
    ink_pixels: usize,
    signature: u64,
}

impl PixelEvidence {
    #[must_use]
    pub const fn ink_pixels(self) -> usize {
        self.ink_pixels
    }

    #[must_use]
    pub const fn signature(self) -> u64 {
        self.signature
    }

    fn from_bgra_bytes(bytes: &[u8]) -> Self {
        let mut signature = 0xcbf2_9ce4_8422_2325_u64;
        let mut ink_pixels = 0;
        for pixel in bytes.chunks_exact(4) {
            let pixel = u32::from_le_bytes([pixel[0], pixel[1], pixel[2], pixel[3]]);
            if pixel & 0x00ff_ffff != 0x00ff_ffff {
                ink_pixels += 1;
            }
            signature ^= u64::from(pixel);
            signature = signature.wrapping_mul(0x0000_0100_0000_01b3);
        }
        Self {
            ink_pixels,
            signature,
        }
    }
}

struct ComApartment;

impl ComApartment {
    fn enter() -> Result<Self, String> {
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }
            .ok()
            .map_err(|error| format!("CoInitializeEx failed: {error}"))?;
        Ok(Self)
    }
}

impl Drop for ComApartment {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}

struct DibCanvas {
    hdc: HDC,
    bitmap: HBITMAP,
    previous_bitmap: HGDIOBJ,
    pixels: *mut u32,
}

impl DibCanvas {
    fn new() -> Result<Self, String> {
        let hdc = unsafe { CreateCompatibleDC(null_mut()) };
        if hdc.is_null() {
            return Err("CreateCompatibleDC failed".into());
        }

        let bitmap_info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: WIDTH,
                biHeight: -HEIGHT,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                ..BITMAPINFOHEADER::default()
            },
            ..BITMAPINFO::default()
        };
        let mut raw_pixels = null_mut::<c_void>();
        let bitmap = unsafe {
            CreateDIBSection(
                hdc,
                &bitmap_info,
                DIB_RGB_COLORS,
                &mut raw_pixels,
                null_mut(),
                0,
            )
        };
        if bitmap.is_null() || raw_pixels.is_null() {
            unsafe {
                DeleteDC(hdc);
            }
            return Err("CreateDIBSection failed".into());
        }

        let previous_bitmap = unsafe { SelectObject(hdc, bitmap) };
        if previous_bitmap.is_null() {
            unsafe {
                DeleteObject(bitmap);
                DeleteDC(hdc);
            }
            return Err("SelectObject(bitmap) failed".into());
        }

        let mut canvas = Self {
            hdc,
            bitmap,
            previous_bitmap,
            pixels: raw_pixels.cast(),
        };
        canvas.clear();
        Ok(canvas)
    }

    fn clear(&mut self) {
        self.pixel_slice_mut().fill(WHITE);
    }

    fn pixel_slice(&self) -> &[u32] {
        unsafe { std::slice::from_raw_parts(self.pixels, PIXEL_COUNT) }
    }

    fn pixel_slice_mut(&mut self) -> &mut [u32] {
        unsafe { std::slice::from_raw_parts_mut(self.pixels, PIXEL_COUNT) }
    }

    fn evidence(&self) -> PixelEvidence {
        let mut signature = 0xcbf2_9ce4_8422_2325_u64;
        let mut ink_pixels = 0;
        for pixel in self.pixel_slice() {
            if pixel & 0x00ff_ffff != 0x00ff_ffff {
                ink_pixels += 1;
            }
            signature ^= u64::from(*pixel);
            signature = signature.wrapping_mul(0x0000_0100_0000_01b3);
        }
        PixelEvidence {
            ink_pixels,
            signature,
        }
    }
}

impl Drop for DibCanvas {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.hdc, self.previous_bitmap);
            DeleteObject(self.bitmap);
            DeleteDC(self.hdc);
        }
    }
}

fn log_font(font: &GdiFont) -> LOGFONTW {
    let mut raw = LOGFONTW {
        lfHeight: font.height(),
        lfWeight: font.weight(),
        ..LOGFONTW::default()
    };
    for (destination, source) in raw
        .lfFaceName
        .iter_mut()
        .take(31)
        .zip(font.family().encode_utf16())
    {
        *destination = source;
    }
    raw
}

fn with_selected_gdi_font<T>(
    canvas: &mut DibCanvas,
    font: &GdiFont,
    operation: impl FnOnce(&mut DibCanvas) -> Result<T, String>,
) -> Result<T, String> {
    let raw_font = log_font(font);
    with_selected_log_font(canvas, &raw_font, operation)
}

fn with_selected_log_font<T>(
    canvas: &mut DibCanvas,
    raw_font: &LOGFONTW,
    operation: impl FnOnce(&mut DibCanvas) -> Result<T, String>,
) -> Result<T, String> {
    let handle = unsafe { CreateFontIndirectW(raw_font) };
    if handle.is_null() {
        return Err("CreateFontIndirectW failed".into());
    }
    let previous_font = unsafe { SelectObject(canvas.hdc, handle) };
    if previous_font.is_null() {
        unsafe {
            DeleteObject(handle);
        }
        return Err("SelectObject(font) failed".into());
    }

    let result = operation(canvas);
    unsafe {
        SelectObject(canvas.hdc, previous_font);
        DeleteObject(handle);
    }
    result
}

fn draw_gdi(canvas: &mut DibCanvas, call: PreparedGdiCall) -> Result<(), String> {
    with_selected_gdi_font(canvas, call.font(), |canvas| {
        unsafe {
            SetBkMode(canvas.hdc, TRANSPARENT as i32);
            SetTextColor(canvas.hdc, 0);
        }
        let spacing = call.spacing().map_or(null(), <[i32]>::as_ptr);
        let succeeded = unsafe {
            ExtTextOutW(
                canvas.hdc,
                12,
                18,
                call.options(),
                null(),
                call.units().as_ptr(),
                call.units().len() as u32,
                spacing,
            )
        };
        if succeeded == 0 {
            Err("ExtTextOutW failed".into())
        } else {
            Ok(())
        }
    })
}

fn glyph_call(
    canvas: &mut DibCanvas,
    text: &str,
    font: GdiFont,
) -> Result<(GdiCall, GdiGlyphMap), String> {
    let units: Vec<u16> = text.encode_utf16().collect();
    let glyphs = with_selected_gdi_font(canvas, &font, |canvas| {
        let mut glyphs = vec![0_u16; units.len()];
        let result = unsafe {
            GetGlyphIndicesW(
                canvas.hdc,
                units.as_ptr(),
                units.len() as i32,
                glyphs.as_mut_ptr(),
                GGI_MARK_NONEXISTING_GLYPHS,
            )
        };
        if result == u32::MAX {
            Err("GetGlyphIndicesW failed".into())
        } else {
            Ok(glyphs)
        }
    })?;
    let mappings = glyphs.iter().copied().zip(text.chars()).collect::<Vec<_>>();
    let map = GdiGlyphMap::new(mappings);
    let call = GdiCall::glyph_indices(glyphs, 0, None::<[i32; 0]>, font);
    Ok((call, map))
}

pub fn render_gdi_unicode(decision: RenderDecision) -> Result<PixelEvidence, String> {
    let mut canvas = DibCanvas::new()?;
    let call = GdiCall::unicode(
        "Open",
        0,
        None::<[i32; 0]>,
        GdiFont::new("Segoe UI", -30, 400),
    );
    GdiInlineAdapter::new().invoke(
        call,
        None,
        |_| Ok(decision),
        |prepared| draw_gdi(&mut canvas, prepared),
    )?;
    Ok(canvas.evidence())
}

pub fn render_raw_gdi_unicode(text: &str) -> Result<PixelEvidence, String> {
    let mut canvas = DibCanvas::new()?;
    let units = text.encode_utf16().collect::<Vec<_>>();
    with_selected_gdi_font(&mut canvas, &GdiFont::new("Segoe UI", -30, 400), |canvas| {
        unsafe {
            SetBkMode(canvas.hdc, TRANSPARENT as i32);
            SetTextColor(canvas.hdc, 0);
        }
        if unsafe {
            ExtTextOutW(
                canvas.hdc,
                12,
                18,
                0,
                null(),
                units.as_ptr(),
                units.len() as u32,
                null(),
            )
        } == 0
        {
            Err("ExtTextOutW failed".into())
        } else {
            Ok(())
        }
    })?;
    Ok(canvas.evidence())
}

pub fn render_raw_gdi_symbol(text: &str) -> Result<PixelEvidence, String> {
    let mut canvas = DibCanvas::new()?;
    let units = text.encode_utf16().collect::<Vec<_>>();
    let mut raw_font = log_font(&GdiFont::new("Symbol", -30, 400));
    raw_font.lfCharSet = SYMBOL_CHARSET;
    with_selected_log_font(&mut canvas, &raw_font, |canvas| {
        unsafe {
            SetBkMode(canvas.hdc, TRANSPARENT as i32);
            SetTextColor(canvas.hdc, 0);
        }
        if unsafe {
            ExtTextOutW(
                canvas.hdc,
                12,
                18,
                0,
                null(),
                units.as_ptr(),
                units.len() as u32,
                null(),
            )
        } == 0
        {
            Err("ExtTextOutW failed".into())
        } else {
            Ok(())
        }
    })?;
    Ok(canvas.evidence())
}

pub fn render_raw_text_out(text: &str) -> Result<PixelEvidence, String> {
    let mut canvas = DibCanvas::new()?;
    let units = text.encode_utf16().collect::<Vec<_>>();
    with_selected_gdi_font(&mut canvas, &GdiFont::new("Segoe UI", -30, 400), |canvas| {
        unsafe {
            SetBkMode(canvas.hdc, TRANSPARENT as i32);
            SetTextColor(canvas.hdc, 0);
        }
        if unsafe { TextOutW(canvas.hdc, 12, 18, units.as_ptr(), units.len() as i32) } == 0 {
            Err("TextOutW failed".into())
        } else {
            Ok(())
        }
    })?;
    Ok(canvas.evidence())
}

pub fn render_raw_draw_text(text: &str) -> Result<PixelEvidence, String> {
    let mut canvas = DibCanvas::new()?;
    let units = text.encode_utf16().collect::<Vec<_>>();
    with_selected_gdi_font(&mut canvas, &GdiFont::new("Segoe UI", -30, 400), |canvas| {
        unsafe {
            SetBkMode(canvas.hdc, TRANSPARENT as i32);
            SetTextColor(canvas.hdc, 0);
        }
        let mut rect = RECT {
            left: 12,
            top: 18,
            right: WIDTH - 12,
            bottom: HEIGHT - 12,
        };
        if unsafe {
            DrawTextW(
                canvas.hdc,
                units.as_ptr(),
                units.len() as i32,
                &mut rect,
                DT_LEFT | DT_SINGLELINE,
            )
        } == 0
        {
            Err("DrawTextW failed".into())
        } else {
            Ok(())
        }
    })?;
    Ok(canvas.evidence())
}

fn render_gdi_glyph_indices_with_font(
    font: GdiFont,
    decision: RenderDecision,
) -> Result<PixelEvidence, String> {
    let mut canvas = DibCanvas::new()?;
    let (call, map) = glyph_call(&mut canvas, "Open", font)?;
    GdiInlineAdapter::new().invoke(
        call,
        Some(&map),
        |_| Ok(decision),
        |prepared| draw_gdi(&mut canvas, prepared),
    )?;
    Ok(canvas.evidence())
}

pub fn render_gdi_glyph_indices(decision: RenderDecision) -> Result<PixelEvidence, String> {
    render_gdi_glyph_indices_with_font(GdiFont::new("Segoe UI", -30, 400), decision)
}

pub fn render_raw_gdi_glyph_indices() -> Result<PixelEvidence, String> {
    render_gdi_glyph_indices_with_font(
        GdiFont::new("Microsoft YaHei UI", -30, 400),
        RenderDecision {
            text: TextDecision::Keep,
            font: FontDecision::Keep,
            generation: Generation::new(0),
        },
    )
}

struct GdiPlusToken(usize);

impl GdiPlusToken {
    fn start() -> Result<Self, String> {
        let input = GdiplusStartupInput {
            GdiplusVersion: 1,
            DebugEventCallback: 0,
            SuppressBackgroundThread: 0,
            SuppressExternalCodecs: 0,
        };
        let mut token = 0;
        let status = unsafe { GdiplusStartup(&mut token, &input, null_mut()) };
        check_status(status, "GdiplusStartup")?;
        Ok(Self(token))
    }
}

impl Drop for GdiPlusToken {
    fn drop(&mut self) {
        unsafe {
            GdiplusShutdown(self.0);
        }
    }
}

struct Graphics(*mut GpGraphics);

impl Drop for Graphics {
    fn drop(&mut self) {
        unsafe {
            GdipDeleteGraphics(self.0);
        }
    }
}

struct FontFamily(*mut GpFontFamily);

impl Drop for FontFamily {
    fn drop(&mut self) {
        unsafe {
            GdipDeleteFontFamily(self.0);
        }
    }
}

struct Font(*mut GpFont);

impl Drop for Font {
    fn drop(&mut self) {
        unsafe {
            GdipDeleteFont(self.0);
        }
    }
}

struct Brush(*mut GpSolidFill);

impl Drop for Brush {
    fn drop(&mut self) {
        unsafe {
            GdipDeleteBrush(self.0.cast::<GpBrush>());
        }
    }
}

fn check_status(status: i32, operation: &str) -> Result<(), String> {
    if status == 0 {
        Ok(())
    } else {
        Err(format!("{operation} failed with status {status}"))
    }
}

fn wide_null(text: &str) -> Vec<u16> {
    text.encode_utf16().chain([0]).collect()
}

fn draw_gdiplus(canvas: &mut DibCanvas, call: PreparedGdiPlusCall) -> Result<(), String> {
    let _token = GdiPlusToken::start()?;
    let mut raw_graphics = null_mut();
    check_status(
        unsafe { GdipCreateFromHDC(canvas.hdc, &mut raw_graphics) },
        "GdipCreateFromHDC",
    )?;
    let graphics = Graphics(raw_graphics);
    check_status(
        unsafe { GdipGraphicsClear(graphics.0, WHITE) },
        "GdipGraphicsClear",
    )?;

    let family_name = wide_null(call.font().family());
    let mut raw_family = null_mut();
    check_status(
        unsafe { GdipCreateFontFamilyFromName(family_name.as_ptr(), null_mut(), &mut raw_family) },
        "GdipCreateFontFamilyFromName",
    )?;
    let family = FontFamily(raw_family);

    let mut raw_font = null_mut();
    check_status(
        unsafe {
            GdipCreateFont(
                family.0,
                call.font().size(),
                call.font().style(),
                call.font().unit(),
                &mut raw_font,
            )
        },
        "GdipCreateFont",
    )?;
    let font = Font(raw_font);

    let mut raw_brush = null_mut();
    check_status(
        unsafe { GdipCreateSolidFill(BLACK, &mut raw_brush) },
        "GdipCreateSolidFill",
    )?;
    let brush = Brush(raw_brush);
    let layout = RectF {
        X: 12.0,
        Y: 12.0,
        Width: (WIDTH - 24) as f32,
        Height: (HEIGHT - 24) as f32,
    };
    check_status(
        unsafe {
            GdipDrawString(
                graphics.0,
                call.units().as_ptr(),
                call.units().len() as i32,
                font.0,
                &layout,
                null(),
                brush.0.cast::<GpBrush>(),
            )
        },
        "GdipDrawString",
    )
}

fn render_gdiplus_text_with_font(
    source: &str,
    font: GdiPlusFont,
    decision: RenderDecision,
) -> Result<PixelEvidence, String> {
    let mut canvas = DibCanvas::new()?;
    let call = GdiPlusCall::utf16(source, font, 11, 12, 13);
    GdiPlusInlineAdapter::new().invoke(
        call,
        |_| Ok(decision),
        |prepared| draw_gdiplus(&mut canvas, prepared),
    )?;
    Ok(canvas.evidence())
}

pub fn render_gdiplus_text(
    source: &str,
    decision: RenderDecision,
) -> Result<PixelEvidence, String> {
    render_gdiplus_text_with_font(source, GdiPlusFont::new("Segoe UI", 30.0, 0, 2), decision)
}

pub fn render_raw_gdiplus_symbol(text: &str) -> Result<PixelEvidence, String> {
    render_gdiplus_text_with_font(
        text,
        GdiPlusFont::new("Symbol", 30.0, 0, 2),
        RenderDecision {
            text: TextDecision::Keep,
            font: FontDecision::Keep,
            generation: Generation::new(0),
        },
    )
}

pub fn render_raw_gdiplus_unicode(text: &str) -> Result<PixelEvidence, String> {
    render_gdiplus_text(
        text,
        RenderDecision {
            text: TextDecision::Keep,
            font: FontDecision::Keep,
            generation: Generation::new(0),
        },
    )
}

pub fn render_gdiplus(decision: RenderDecision) -> Result<PixelEvidence, String> {
    render_gdiplus_text("Open", decision)
}

fn direct2d_target_properties() -> D2D1_RENDER_TARGET_PROPERTIES {
    D2D1_RENDER_TARGET_PROPERTIES {
        r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
        pixelFormat: D2D1_PIXEL_FORMAT {
            format: DXGI_FORMAT_B8G8R8A8_UNORM,
            alphaMode: D2D1_ALPHA_MODE_IGNORE,
        },
        dpiX: 0.0,
        dpiY: 0.0,
        usage: D2D1_RENDER_TARGET_USAGE_NONE,
        minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
    }
}

fn direct2d_wic_target_properties() -> D2D1_RENDER_TARGET_PROPERTIES {
    let mut properties = direct2d_target_properties();
    properties.pixelFormat.alphaMode = D2D1_ALPHA_MODE_PREMULTIPLIED;
    properties
}

pub fn render_raw_direct2d_text(text: &str) -> Result<PixelEvidence, String> {
    let canvas = DibCanvas::new()?;
    let direct2d: ID2D1Factory =
        unsafe { D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None) }
            .map_err(|error| format!("D2D1CreateFactory failed: {error}"))?;
    let target = unsafe { direct2d.CreateDCRenderTarget(&direct2d_target_properties()) }
        .map_err(|error| format!("CreateDCRenderTarget failed: {error}"))?;
    let bounds = WindowsRect {
        left: 0,
        top: 0,
        right: WIDTH,
        bottom: HEIGHT,
    };
    unsafe { target.BindDC(WindowsHdc(canvas.hdc), &bounds) }
        .map_err(|error| format!("ID2D1DCRenderTarget::BindDC failed: {error}"))?;

    let directwrite: IDWriteFactory = unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED) }
        .map_err(|error| format!("DWriteCreateFactory failed: {error}"))?;
    let format = unsafe {
        directwrite.CreateTextFormat(
            w!("Segoe UI"),
            None,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_FONT_STYLE_NORMAL,
            DWRITE_FONT_STRETCH_NORMAL,
            30.0,
            w!("en-US"),
        )
    }
    .map_err(|error| format!("CreateTextFormat failed: {error}"))?;
    let brush = unsafe {
        target.CreateSolidColorBrush(
            &D2D1_COLOR_F {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            None,
        )
    }
    .map_err(|error| format!("CreateSolidColorBrush failed: {error}"))?;
    let units = text.encode_utf16().collect::<Vec<_>>();
    let layout = D2D_RECT_F {
        left: 12.0,
        top: 12.0,
        right: (WIDTH - 12) as f32,
        bottom: (HEIGHT - 12) as f32,
    };
    let white = D2D1_COLOR_F {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };
    unsafe {
        target.BeginDraw();
        target.Clear(Some(&white));
        target.DrawText(
            &units,
            &format,
            &layout,
            &brush,
            D2D1_DRAW_TEXT_OPTIONS_NONE,
            DWRITE_MEASURING_MODE_NATURAL,
        );
        target.EndDraw(None, None)
    }
    .map_err(|error| format!("ID2D1RenderTarget::EndDraw failed: {error}"))?;
    Ok(canvas.evidence())
}

pub fn render_raw_directwrite_layout(text: &str) -> Result<PixelEvidence, String> {
    render_raw_directwrite_formatted_layout(text, false, DirectWriteLayoutStyle::Uniform)
}

pub fn render_raw_directwrite_compatible_layout(text: &str) -> Result<PixelEvidence, String> {
    render_raw_directwrite_formatted_layout(text, true, DirectWriteLayoutStyle::Uniform)
}

#[derive(Clone, Copy, Debug)]
pub enum DirectWriteLayoutStyle {
    Uniform,
    Typography,
    UpdatedUniformTypography,
    LocalFontSize,
    LocalTypography,
    LocalCharacterSpacing,
    ReusedEmpty,
}

pub fn render_raw_directwrite_formatted_layout(
    text: &str,
    compatible: bool,
    style: DirectWriteLayoutStyle,
) -> Result<PixelEvidence, String> {
    render_raw_directwrite_layout_sequence(text, compatible, style, 1, |_| {})
        .map(|mut frames| frames.remove(0))
}

pub fn render_raw_directwrite_layout_sequence(
    text: &str,
    compatible: bool,
    style: DirectWriteLayoutStyle,
    frames: usize,
    mut before_draw: impl FnMut(usize),
) -> Result<Vec<PixelEvidence>, String> {
    let canvas = DibCanvas::new()?;
    let direct2d: ID2D1Factory =
        unsafe { D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None) }
            .map_err(|error| format!("D2D1CreateFactory failed: {error}"))?;
    let parent = unsafe { direct2d.CreateDCRenderTarget(&direct2d_target_properties()) }
        .map_err(|error| format!("CreateDCRenderTarget failed: {error}"))?;
    let bounds = WindowsRect {
        left: 0,
        top: 0,
        right: WIDTH,
        bottom: HEIGHT,
    };
    unsafe { parent.BindDC(WindowsHdc(canvas.hdc), &bounds) }
        .map_err(|error| format!("ID2D1DCRenderTarget::BindDC failed: {error}"))?;
    let parent_target: ID2D1RenderTarget = parent
        .cast()
        .map_err(|error| format!("ID2D1RenderTarget cast failed: {error}"))?;
    let draw_target = if compatible {
        let target = unsafe {
            parent_target.CreateCompatibleRenderTarget(
                None,
                None,
                None,
                D2D1_COMPATIBLE_RENDER_TARGET_OPTIONS_NONE,
            )
        }
        .map_err(|error| format!("CreateCompatibleRenderTarget failed: {error}"))?;
        target
            .cast::<ID2D1RenderTarget>()
            .map_err(|error| format!("compatible ID2D1RenderTarget cast failed: {error}"))?
    } else {
        parent_target.clone()
    };

    let directwrite: IDWriteFactory = unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED) }
        .map_err(|error| format!("DWriteCreateFactory failed: {error}"))?;
    let format = unsafe {
        directwrite.CreateTextFormat(
            w!("Segoe UI"),
            None,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_FONT_STYLE_NORMAL,
            DWRITE_FONT_STRETCH_NORMAL,
            30.0,
            w!("en-US"),
        )
    }
    .map_err(|error| format!("CreateTextFormat failed: {error}"))?;
    let units = text.encode_utf16().collect::<Vec<_>>();
    let layout = unsafe {
        directwrite.CreateTextLayout(&units, &format, (WIDTH - 24) as f32, (HEIGHT - 24) as f32)
    }
    .map_err(|error| format!("CreateTextLayout failed: {error}"))?;
    let layout = if matches!(style, DirectWriteLayoutStyle::ReusedEmpty) {
        // Retain a batch so the fixture does not depend on the allocator choosing
        // the most recently released object (x86 and x64 choose differently).
        let mut sources = vec![layout];
        for _ in 1..128 {
            sources.push(
                unsafe { directwrite.CreateTextLayout(&units, &format, 0.0, 10000.0) }
                    .map_err(|error| error.to_string())?,
            );
        }
        let addresses = sources.iter().map(Interface::as_raw).collect::<Vec<_>>();
        drop(sources);
        let mut others = Vec::new();
        let mut reused = None;
        for _ in 0..256 {
            let empty = unsafe { directwrite.CreateTextLayout(&[], &format, 0.0, 10000.0) }
                .map_err(|error| error.to_string())?;
            if addresses.contains(&empty.as_raw()) {
                reused = Some(empty);
                break;
            }
            others.push(empty);
        }
        reused.ok_or("DirectWrite did not reuse a released layout for the empty-layout fixture")?
    } else {
        layout
    };
    match style {
        DirectWriteLayoutStyle::Uniform | DirectWriteLayoutStyle::ReusedEmpty => {}
        DirectWriteLayoutStyle::Typography
        | DirectWriteLayoutStyle::UpdatedUniformTypography
        | DirectWriteLayoutStyle::LocalTypography => {
            let typography = unsafe { directwrite.CreateTypography() }
                .map_err(|error| format!("CreateTypography failed: {error}"))?;
            unsafe {
                typography.AddFontFeature(DWRITE_FONT_FEATURE {
                    nameTag: DWRITE_FONT_FEATURE_TAG_STANDARD_LIGATURES,
                    parameter: 0,
                })
            }
            .map_err(|error| format!("AddFontFeature failed: {error}"))?;
            unsafe {
                layout.SetTypography(
                    &typography,
                    DWRITE_TEXT_RANGE {
                        startPosition: 0,
                        length: if matches!(style, DirectWriteLayoutStyle::LocalTypography) {
                            1
                        } else {
                            units.len() as u32
                        },
                    },
                )
            }
            .map_err(|error| format!("SetTypography failed: {error}"))?;
        }
        DirectWriteLayoutStyle::LocalFontSize => {
            unsafe {
                layout.SetFontSize(
                    40.0,
                    DWRITE_TEXT_RANGE {
                        startPosition: 0,
                        length: 1,
                    },
                )
            }
            .map_err(|error| format!("SetFontSize failed: {error}"))?;
        }
        DirectWriteLayoutStyle::LocalCharacterSpacing => {
            let layout = layout
                .cast::<IDWriteTextLayout1>()
                .map_err(|error| error.to_string())?;
            unsafe {
                layout.SetCharacterSpacing(
                    4.0,
                    3.0,
                    0.0,
                    DWRITE_TEXT_RANGE {
                        startPosition: 0,
                        length: 1,
                    },
                )
            }
            .map_err(|error| error.to_string())?;
        }
    }
    if matches!(style, DirectWriteLayoutStyle::UpdatedUniformTypography) {
        let range = DWRITE_TEXT_RANGE {
            startPosition: 0,
            length: units.len() as u32,
        };
        (|| unsafe {
            layout.SetFontFamilyName(w!("Arial"), range)?;
            layout.SetLocaleName(w!("zh-CN"), range)?;
            layout.SetFontSize(24.0, range)?;
            layout.SetFontWeight(DWRITE_FONT_WEIGHT_BOLD, range)?;
            layout.SetUnderline(true, range)?;
            layout.SetStrikethrough(true, range)?;
            layout.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_TRAILING)?;
            layout.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
            layout.SetMaxWidth(280.0)?;
            layout.SetMaxHeight(64.0)?;
            let extended = layout.cast::<IDWriteTextLayout1>()?;
            extended.SetPairKerning(true, range)?;
            extended.SetCharacterSpacing(1.0, 2.0, 0.0, range)
        })()
        .map_err(|error: windows::core::Error| {
            format!("Update TextLayout formatting failed: {error}")
        })?;
    }
    let brush = unsafe {
        draw_target.CreateSolidColorBrush(
            &D2D1_COLOR_F {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            None,
        )
    }
    .map_err(|error| format!("CreateSolidColorBrush failed: {error}"))?;
    let white = D2D1_COLOR_F {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };
    let mut evidence = Vec::with_capacity(frames);
    for index in 0..frames {
        before_draw(index);
        unsafe {
            draw_target.BeginDraw();
            draw_target.Clear(Some(&white));
            draw_target.DrawTextLayout(
                D2D_POINT_2F { x: 12.0, y: 12.0 },
                &layout,
                &brush,
                D2D1_DRAW_TEXT_OPTIONS_NONE,
            );
            draw_target.EndDraw(None, None)
        }
        .map_err(|error| format!("TextLayout EndDraw failed: {error}"))?;

        if compatible {
            let bitmap_target = draw_target
                .cast::<windows::Win32::Graphics::Direct2D::ID2D1BitmapRenderTarget>()
                .map_err(|error| format!("ID2D1BitmapRenderTarget cast failed: {error}"))?;
            let bitmap = unsafe { bitmap_target.GetBitmap() }
                .map_err(|error| format!("GetBitmap failed: {error}"))?;
            let destination = D2D_RECT_F {
                left: 0.0,
                top: 0.0,
                right: WIDTH as f32,
                bottom: HEIGHT as f32,
            };
            unsafe {
                parent_target.BeginDraw();
                parent_target.Clear(Some(&white));
                parent_target.DrawBitmap(
                    &bitmap,
                    Some(&destination),
                    1.0,
                    D2D1_BITMAP_INTERPOLATION_MODE_NEAREST_NEIGHBOR,
                    None,
                );
                parent_target.EndDraw(None, None)
            }
            .map_err(|error| format!("compatible copy EndDraw failed: {error}"))?;
        }
        evidence.push(canvas.evidence());
    }
    Ok(evidence)
}

pub fn render_raw_direct2d_wic_text(text: &str) -> Result<PixelEvidence, String> {
    let _apartment = ComApartment::enter()?;
    let imaging: IWICImagingFactory =
        unsafe { CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER) }
            .map_err(|error| format!("CoCreateInstance(WIC) failed: {error}"))?;
    let bitmap = unsafe {
        imaging.CreateBitmap(
            WIDTH as u32,
            HEIGHT as u32,
            &GUID_WICPixelFormat32bppPBGRA,
            WICBitmapCacheOnLoad,
        )
    }
    .map_err(|error| format!("IWICImagingFactory::CreateBitmap failed: {error}"))?;
    let direct2d: ID2D1Factory =
        unsafe { D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None) }
            .map_err(|error| format!("D2D1CreateFactory failed: {error}"))?;
    let target =
        unsafe { direct2d.CreateWicBitmapRenderTarget(&bitmap, &direct2d_wic_target_properties()) }
            .map_err(|error| format!("CreateWicBitmapRenderTarget failed: {error}"))?;
    let directwrite: IDWriteFactory = unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED) }
        .map_err(|error| format!("DWriteCreateFactory failed: {error}"))?;
    let format = unsafe {
        directwrite.CreateTextFormat(
            w!("Segoe UI"),
            None,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_FONT_STYLE_NORMAL,
            DWRITE_FONT_STRETCH_NORMAL,
            30.0,
            w!("en-US"),
        )
    }
    .map_err(|error| format!("CreateTextFormat failed: {error}"))?;
    let brush = unsafe {
        target.CreateSolidColorBrush(
            &D2D1_COLOR_F {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            None,
        )
    }
    .map_err(|error| format!("CreateSolidColorBrush failed: {error}"))?;
    let units = text.encode_utf16().collect::<Vec<_>>();
    let layout = D2D_RECT_F {
        left: 12.0,
        top: 12.0,
        right: (WIDTH - 12) as f32,
        bottom: (HEIGHT - 12) as f32,
    };
    let white = D2D1_COLOR_F {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };
    unsafe {
        target.BeginDraw();
        target.Clear(Some(&white));
        target.DrawText(
            &units,
            &format,
            &layout,
            &brush,
            D2D1_DRAW_TEXT_OPTIONS_NONE,
            DWRITE_MEASURING_MODE_NATURAL,
        );
        target.EndDraw(None, None)
    }
    .map_err(|error| format!("ID2D1RenderTarget::EndDraw failed: {error}"))?;

    let mut pixels = vec![0_u8; PIXEL_COUNT * 4];
    unsafe { bitmap.CopyPixels(std::ptr::null(), (WIDTH * 4) as u32, &mut pixels) }
        .map_err(|error| format!("IWICBitmap::CopyPixels failed: {error}"))?;
    Ok(PixelEvidence::from_bgra_bytes(&pixels))
}

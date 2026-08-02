//! Windows synthetic drawing host for first-party Adapter acceptance.

#[cfg(windows)]
mod windows {
    use glyphshift_adapter_gdi::{
        GdiCall, GdiFont, GdiGlyphMap, GdiInlineAdapter, PreparedGdiCall,
    };
    use glyphshift_adapter_gdiplus::{
        GdiPlusCall, GdiPlusFont, GdiPlusInlineAdapter, PreparedGdiPlusCall,
    };
    use glyphshift_domain::RenderDecision;
    use std::ffi::c_void;
    use std::mem::size_of;
    use std::ptr::{null, null_mut};
    use windows_sys::Win32::Foundation::RECT;
    use windows_sys::Win32::Graphics::Gdi::{
        CreateCompatibleDC, CreateDIBSection, CreateFontIndirectW, DeleteDC, DeleteObject,
        DrawTextW, ExtTextOutW, GetGlyphIndicesW, SelectObject, SetBkMode, SetTextColor, TextOutW,
        BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, DT_LEFT, DT_SINGLELINE,
        GGI_MARK_NONEXISTING_GLYPHS, HBITMAP, HDC, HGDIOBJ, LOGFONTW, TRANSPARENT,
    };
    use windows_sys::Win32::Graphics::GdiPlus::{
        GdipCreateFont, GdipCreateFontFamilyFromName, GdipCreateFromHDC, GdipCreateSolidFill,
        GdipDeleteBrush, GdipDeleteFont, GdipDeleteFontFamily, GdipDeleteGraphics, GdipDrawString,
        GdipGraphicsClear, GdiplusShutdown, GdiplusStartup, GdiplusStartupInput, GpBrush, GpFont,
        GpFontFamily, GpGraphics, GpSolidFill, RectF,
    };

    const WIDTH: i32 = 360;
    const HEIGHT: i32 = 96;
    const PIXEL_COUNT: usize = WIDTH as usize * HEIGHT as usize;
    const WHITE: u32 = 0xffff_ffff;
    const BLACK: u32 = 0xff00_0000;

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
        let handle = unsafe { CreateFontIndirectW(&raw_font) };
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

    pub fn render_gdi_glyph_indices(decision: RenderDecision) -> Result<PixelEvidence, String> {
        let mut canvas = DibCanvas::new()?;
        let (call, map) = glyph_call(&mut canvas, "Open", GdiFont::new("Segoe UI", -30, 400))?;
        GdiInlineAdapter::new().invoke(
            call,
            Some(&map),
            |_| Ok(decision),
            |prepared| draw_gdi(&mut canvas, prepared),
        )?;
        Ok(canvas.evidence())
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
            unsafe {
                GdipCreateFontFamilyFromName(family_name.as_ptr(), null_mut(), &mut raw_family)
            },
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

    pub fn render_gdiplus_text(
        source: &str,
        decision: RenderDecision,
    ) -> Result<PixelEvidence, String> {
        let mut canvas = DibCanvas::new()?;
        let call = GdiPlusCall::utf16(source, GdiPlusFont::new("Segoe UI", 30.0, 0, 2), 11, 12, 13);
        GdiPlusInlineAdapter::new().invoke(
            call,
            |_| Ok(decision),
            |prepared| draw_gdiplus(&mut canvas, prepared),
        )?;
        Ok(canvas.evidence())
    }

    pub fn render_gdiplus(decision: RenderDecision) -> Result<PixelEvidence, String> {
        render_gdiplus_text("Open", decision)
    }
}

#[cfg(windows)]
pub use windows::{
    render_gdi_glyph_indices, render_gdi_unicode, render_gdiplus, render_gdiplus_text,
    render_raw_draw_text, render_raw_gdi_unicode, render_raw_text_out, PixelEvidence,
};

//! Windows synthetic drawing host for first-party Adapter acceptance.

#[cfg(windows)]
mod windows;

#[cfg(windows)]
pub use windows::{
    render_gdi_glyph_indices, render_gdi_unicode, render_gdiplus, render_gdiplus_text,
    render_raw_direct2d_text, render_raw_direct2d_wic_text,
    render_raw_directwrite_compatible_layout, render_raw_directwrite_layout, render_raw_draw_text,
    render_raw_gdi_glyph_indices, render_raw_gdi_symbol, render_raw_gdi_unicode,
    render_raw_gdiplus_symbol, render_raw_gdiplus_unicode, render_raw_text_out,
    run_uia_standard_control_server, write_raw_console, PixelEvidence,
};

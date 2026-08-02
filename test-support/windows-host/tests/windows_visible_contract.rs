#![cfg(windows)]

use glyphshift_domain::{FontDecision, Generation, RenderDecision, TextDecision};
use glyphshift_windows_host::{
    render_gdi_glyph_indices, render_gdi_unicode, render_gdiplus, PixelEvidence,
};

fn keep() -> RenderDecision {
    RenderDecision {
        text: TextDecision::Keep,
        font: FontDecision::Keep,
        generation: Generation::new(1),
    }
}

fn replace() -> RenderDecision {
    RenderDecision {
        text: TextDecision::Replace("Localized UI".into()),
        font: FontDecision::Substitute("Arial".into()),
        generation: Generation::new(2),
    }
}

fn assert_visible_change(original: PixelEvidence, replacement: PixelEvidence) {
    assert!(original.ink_pixels() > 0, "original call must draw pixels");
    assert!(
        replacement.ink_pixels() > 0,
        "replacement call must draw pixels"
    );
    assert_ne!(
        original.signature(),
        replacement.signature(),
        "replacement must change the in-memory pixel surface"
    );
}

#[test]
fn windows_gdi_unicode_and_current_font_glyph_paths_change_visible_pixels() {
    assert_visible_change(
        render_gdi_unicode(keep()).expect("render original GDI text"),
        render_gdi_unicode(replace()).expect("render replacement GDI text"),
    );
    assert_visible_change(
        render_gdi_glyph_indices(keep()).expect("render original GDI glyphs"),
        render_gdi_glyph_indices(replace()).expect("render replacement GDI glyphs"),
    );
}

#[test]
fn windows_gdiplus_path_changes_visible_pixels() {
    assert_visible_change(
        render_gdiplus(keep()).expect("render original GDI+ text"),
        render_gdiplus(replace()).expect("render replacement GDI+ text"),
    );
}
